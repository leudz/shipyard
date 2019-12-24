# Unique Storage

When it's known that there will only ever be exactly one instance of some component data, it doesn't actually need to be part of the ECS since it's more like global data

However, making it available to systems and world runs is very convenient.

Use cases: Window Size, Audio Device, even Camera