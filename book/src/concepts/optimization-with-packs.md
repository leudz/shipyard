# Optimization with packs

Packs provide a way to align data that is known to travel together. 

By packing components together, they can be iterated over with the usual Shipyard semantics, but under the hood they avoid cache misses completely.

However, not all components can be packed together all the time. Sometimes they need to be viewed in different groups.

_Loose Packs vs. Tight Packs..._