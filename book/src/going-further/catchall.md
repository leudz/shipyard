# Catchall

Catchall gather small small chapters, they are still important though.

## Might want to try_*

## GetComponent and Copy

## `!Send` and `!Sync`

`World` can store `!Send` and/or `!Sync` components but they come with limitations.

- `!Send` storages can only be added in `World`'s thread.
- `Send + !Sync` components can only be accessed from one thread at a time.
- `!Send + Sync` components can only be accessed immutably from other threads.
- `!Send + !Sync` components can only be accessed in the thread they were added in.

With these constrains you can run into issues like unrunnable systems or undeletable entities.
As a rule of thumb, try to call `World::run_default` and `run_workload` from `World`'s thread.

To help with `!Send` storages, all systems borrowing `AllStorages` will run in the thread `World::run_default` or `run_workload` is called in.

## Unique Storage

When it's known that there will only ever be exactly one instance of some component, it doesn't need to be attached to an entity.

It also work well as global data while still been sound.

Use cases: Window Size, Audio Device, even Camera
