# Planning for Version 2.0

## External

 * All requests need to have a timeout. Default to 1s.
 * Re-implement modbus on top of bytestream
 * Instruments may contain a list of handleids?
 * Modbus requests all carry the protocol (not in address)
 * Move VISA into own library
 * Remove address <-> request separation? The Address is just an enum fitting to the instrument. Hence it's encoded in the type system

## Internal Refactoring

 * Create self-reconnecting streams
 * Somehow improve Address <-> Instrument matching? Or avoid it from start by creating multiple inventories?

## Other points

 * Collect issues about error reporting
 * Collect requirements and implement based on requirements
 * Think about how to handle the `check_alive()` issue for scpi instruments.

## Open Questions

 * How to handle prologix???
 * How to migrate?
    - Probably on python level, automatically switch based on detected comsrv version
