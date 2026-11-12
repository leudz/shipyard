# Grouping retrospective

Grouping is a way to order components across storages to improve iteration speed at the cost of insertion/removal.

## The problem

Shipyard's default iteration picks a storage and then checks if other storages also have a component for that entity.

```txt
A: [e0 e1 e2 e3]
B: [e9 e4 e7 e2 e1]
```

For example here with an `(A, B)` query, we'd pick `A` to drive.\
Then for each of its components, we'd check if the entity also has a `B` component.\
`e0`? No. `e1`? Yes. `e2`? Yes. `e3`? No.

Grouping aims to align components across multiple storages and keep them ordered as time goes on.\
We want `e1` and `e2`'s `A` and `B` components to have this form:

```txt
A: [... e1 e2 ...]
B: [... e1 e2 ...]
```

The grouped components are present in the same entity order in each storage.\
This allows iterating them without any lookup.

## Pack

Let's go back to the first iteration, the year: 2019.

Shipyard was influenced by [EnTT](https://github.com/skypjack/entt), one of the main C++ ECS libraries.\
The author, Skypjack, has a blog where he explains many internal designs.\
Grouping is one of them. https://skypjack.github.io/2019-03-21-ecs-baf-part-2-insights/

I didn't want to use the same name, so I went with "packing" at the time.\
Grouping, as explained in the blog post, has different variants.\
In Shipyard 0.1, only the full-owning variant was available.\
0.3 added partial-owning and they were both taken out in 0.5.\
The release note said "Packs are removed temporarily". It took me a bit more time than I thought.

A big difference between EnTT and Shipyard is the language they are written in.\
Rust enforces strong guarantees around mutation. Shipyard modeled `View` and `ViewMut` based on shared or exclusive access at the storage level.

Keeping components aligned across storages doesn't play well with borrowing storage independently.\
Let's say I have an `A+B` group, I borrow `View<A>` and `ViewMut<B>`.\
With `ViewMut` I can add and remove `B` components. How can I keep `A` synced then?

0.1->0.4 solved this problem by requiring all storages from the group when adding or removing.\
The syntax was particularly ugly for removals since the component(s) had to be specified with a generic.\
`Remove::<(B,)>::((&mut vm_a, &mut vm_b), entity_id)`\
Archetype ECS libraries and [Sparsey](https://github.com/LechintanTudor/sparsey) for example solve this by not allowing independent storage borrow.

## 2026

I never gave up on grouping. Like I said in the 0.5 release note, it was always supposed to come back.\
Throughout the years, I had ideas but never one that motivated me enough.\
The one or two times I did start, I realized just how much work it'd be and stopped.

In April 2026, [JWSong](https://github.com/JWSong) opened [an issue to "explore opt-in grouped iteration"](https://github.com/leudz/shipyard/issues/235).\
I shared the ideas I had at the time and warned that it'd be a lot of work.\
In hindsight, having someone else implement it would have required way too much back-and-forth over design choices.

### Multiple vectors

A few months later, I started implementing the solution I had in mind.

To fix the issue packing had, this version is lazy.\
Adding and removing components doesn't immediately restore group alignment.\
Inserting a component first puts it in an `unevaluated` state.\
Another function then restores the grouped components order.\
This split allows the latter function to borrow the full set of grouped storages.

Each group would have a separate vector to hold ordered components in every storage of the group.\
It allows any group combinations without needing multiple kinds of groups like EnTT.\
This freedom comes with the price of fragmenting the components like in archetypes.

Let's go back to our previous example, we start with:

```txt
A: [e0 e1 e2 e3]
B: [e9 e4 e7 e2 e1]
```

Then we extract grouped entities:

```txt
A: [e0 e3]    [e1 e2]
B: [e9 e4 e7] [e1 e2]
```

So far, so good.

#### Removal

Let's try to remove a grouped component, `e1`'s `B`. 

```txt
A: [e0 e3]    [e1 e2]
B: [e9 e4 e7] [XX e2]
```

What do we do with the hole now?\
Without group, `SparseSet::remove` uses `Vec::swap_remove`. Here, it would mess with component alignment.

```txt
A: [e0 e3]    [e1 e2]
B: [e9 e4 e7] [e2]
```

`Vec::remove` wouldn't shuffle `B` but would still break alignment with `A` the same way.

What I ended up doing was having a bitset for each grouped vector.

```txt
A: [e0 e3]    [e1 e2]
               1  1
B: [e9 e4 e7] [XX e2]
               0  1
```

The holes stay in place until the `regroup` function is called.\
For each hole, it extracts the components from the other storages and puts them back in the non grouped vector.

```txt
A: [e0 e3 e1] [e2]
B: [e9 e4 e7] [e2]
```

This is good but our iterator cannot assume `regroup` was just called.\
Even with non grouped iteration, we have to check the bitset to make sure the component wasn't removed recently.\
In my tests, this made non grouped iteration significantly slower.

I gave up on this design at that point.\
A solution to the bitset issue was right around the corner however.

### Single vector, multiple ranges

I decided to scrap everything and go back to one vector.\
I still wanted to allow any group combinations with a single grouping kind.\
So let's go with another design: multiple ranges.

We'll add a third component type to our example and a few more components.\
This time we'll have `A+B` and `A+C` groups.

```txt
A: [e0 e1 e2 e3 e4 e5 e6]
B: [e9 e4 e7 e2 e1]
C: [e5 e3 e6]
```

We keep the lazy API from the previous design.\
After a `regroup`, we get:

```txt
A: [e1 e2 e4 e3 e5 e6 e0]
    |......| |......|

B: [e1 e2 e4 e9 e7]
    |......|

C: [e3 e5 e6]
    |......|
```

`A` gets two ranges, one for `A+B` and one for `A+C`.\
`B` and `C` get a single range each.\
Inside a group range, components have the same entity order across storages but they may have a different start index.\
We also have the `unevaluated` range at the very end, which is empty here since we just called `regroup`.\
And `ungrouped` with `e0`, `e9` and `e7` for components that have gone through `regroup` but didn't match any group.\
We need this distinction so repeatedly calling `regroup` doesn't check components again and again.

This new design doesn't solve anything so far, it only piles up more problems.\
Removing a component from a group has never been more complex.\
We definitely can't leave a hole in the only vector this time.

Let's try to remove `e2`'s `A` component.

```txt
A: [e1 XX e4 e3 e5 e6 e0]
    |......| |......|
```

#### `Vec::swap_remove`

The classic `SparseSet::remove` gets us:

```txt
A: [e1 e0 e4 e3 e5 e6]
    |......| |......|
```

`e0` moved to `e2`'s old position.\
`e0` is not part of `A+B` though so it has to shrink.

```txt
A: [e1 e0 e4 e3 e5 e6]
    ||       |......|
```

Now we lost the information that `e4` is part of `A+B`.\
Plus `e0` was categorized as `ungrouped`, that's also lost.

We can swap `e0` and `e4` to get back `e4` information.

```txt
A: [e1 e4 e0 e3 e5 e6]
    |...|    |......|
```

For `e0`, it's more complicated.\
We could track these in-between components, performance-wise it wouldn't be too bad.\
Splitting `ungrouped` and/or `unevaluated` components has consequences however.\
Iterators can use group information but also `ungrouped` and `unevaluated`. Having them all over the storage would make it very complex.

We could also delete all group information, or at least everything at the right of the removal.\
That would make `regroup` a lot more costly.

#### `Vec::remove`

So a simple `Vec::swap_remove` is out, what about `Vec::remove`?

```txt
A: [e1 e4 e3 e5 e6 e0]
    |...| |......|
```

That's a pretty good state.\
We don't shuffle anything. `e4` is still inside `A+B` range and `e0` is at the end in the `ungrouped` range.

We have a bit of a problem with `A+B`, like with multiple vectors.

```txt
A: [e1 e4 e3 e5 e6 e0]
    |...| |......|

B: [e1 e2 e4 e9 e7]
    |......|
```

While both storages have at least 2 components, they are no longer aligned.\
`B` would still have `e2` at index 1 where `A` has `e4`.

On the bright side, `e1` hasn't changed. We could still iterate that part without checking anything.\
What if we introduced another range, within the existing group range, for these components.\
A trusted length.\
As soon as a position within the group changes, the trusted length stops there.\
When iterating, we can use the minimal trusted length of all iterated storages as a stopping point.\
For the rest, we simply use the slower checked iteration.

When we removed `e2`, we touched `A` at offset 1 in the `A+B` group so the trusted length drops to 1.\
`B` and `C` are unaffected.

```txt
A:                 [e1 e4 e3 e5 e6 e0]
A+B Group:          |...|
A+B Trusted length: ||
A+C Group:                |......|
A+C Trusted length:       |......|

B: [e1 e2 e4 e9 e7]
    |......|

C: [e3 e5 e6]
    |......|
```

We can now iterate `(A, B)` safely, we take `min(1, 3)` as `A+B` group fast iteration length and fall back on checked iteration for the rest.\
When we `regroup` next time, we keep the information that `e4` is in `A+B` group too.

---

Trusted length could've been very useful for the multi vectors design.\
It could've been a trusted range, starting somewhere other than 0.\
I revisited the idea after finishing the new attempt.\
It can definitely work, with different trade-offs than what Shipyard has right now.\
Because multi vectors impacts non grouped iteration, I preferred to stay with a single vector.

---

Are we done, then? `Vec::remove` plus trusted length for grouped components is perfect?\
Not quite. Here 5 components were moved but there could be thousands of grouped components.\
`SparseSet::remove` is `O(1)` currently, we don't want to make it `O(number of following grouped components)`.

#### Multiple `Vec::swap_remove`

It definitely has to be `Vec::swap_remove` then.\
Since a single one moves components too far, let's try to keep the swaps local.

```txt
A: [e1 XX e4 e3 e5 e6 e0]
        <->
    |......| |......|

A: [e1 e4 XX e3 e5 e6 e0]
           <------->
    |...|    |......|

A: [e1 e4 e6 e3 e5 XX e0]
                    <->
    |...| |......|

A: [e1 e4 e6 e3 e5 e0]
    |...| |......|
```

We got `e2` out. `e0` is still at the right place.\
We've paid `O(number of groups)`, that's better than `O(number of following grouped components)`.\
The final state is far from ideal though.

```txt
A: [e1 e4 e6 e3 e5 e0]
    |...| |......|

B: [e1 e2 e4 e9 e7]
    |......|

C: [e3 e5 e6]
    |......|
```

Each range owns their correct components, yes.\
The trusted lengths are at an all time low however.\
`A+B` is at 1. `A+C` is at 0 since `e6` moved to the front and changed the order inside `A`.

We could be happy with this. We keep some fast iteration even with removal.\
You can imagine a group composed of 1000 components with the 999th component removed.\
Iteration would remain mainly on the fast path, and that's all while waiting for a `regroup`.

Of course not, we're not happy!\
We can't do much more for `A+B` but `A+C` didn't ask for anything and got its trusted length completely destroyed, that's not fair.

One solution would be to have a trusted range, like hinted at in the multi vectors aside.\
`A` would have `e3` and `e5` trusted, we'd offset the start of the trusted length by 1.\
But technically, `e6` is right there. Is there nothing we can do?

When I saw the ranges over the storage vector, I thought of `VecDeque`.\
Instead of visualizing swaps, picture growable ring buffers. Each range is a ring buffer.\
Since it's a ring buffer, we can iterate it even when it's rotated.\
It's not perfect, but it works.

```txt
A: [e1 e4 e6 e3 e5 e0]
    |...|    |....
          .|

C: [e3 e5 e6]
    |......|
```

`A+C` range inside `A` has a shift of 1. `A+C` inside `C` has a shift of 0.\
Iteration starts with `e3` then `e5`, `A` reaches the end of its buffer and wraps to its start.\
The components might have rotated inside `A` but we still visit them in the same order as before.\
So `A+C` can keep its full trusted length. Iteration is split but it's fast path all the way.\
Further rotations move the split point but don't create new ones.

---

This is Shipyard 0.12 grouping design.

### Regroup

I won't go over the full algorithm, it's not very interesting but there is one important point.\
Earlier I said I wanted to allow any group combination so let's add a `C` component to `e4`.

```txt
A: [e1 e2 e4 e3 e5 e6 e0]
    |......| |......|

B: [e1 e2 e4 e9 e7]
    |......|

C: [e3 e5 e6 e4]
    |......|
```

#### Union

`e4` has both `A` and `B` components already. More importantly, it's in the `A+B` range.\
What should happen after a `regroup`? We can't have the component twice in one storage.

```txt
A: [e1 e2 e4 e3 e5 e6 e4 e0]
    |......| |.........|
```

To solve this problem, we simply borrow from archetypes.\
When an entity falls into multiple ranges, we create a new one.

```txt
A: [e1 e2 e3 e5 e6 e4 e0]
    |...| |......| ||

B: [e1 e2 e4 e9 e7]
    |...| ||

C: [e3 e5 e6 e4]
    |......| ||
```

We now have the `A+B+C` group.\
As opposed to the groups we've seen so far, this one is created automatically.

Just like archetypes, iterators for an `(A, B)` query would visit both `A+B` and `A+B+C` ranges.