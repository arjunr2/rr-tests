## Todos for RR Wasmtime (put in order of priority)

* Zero the values in Entry/Return (this is not super easy to add plus seems unnecessary)
* Serialize configuration somehow?
* Support libcalls (environ/src/components.rs) + (wasmtime/src/runtime/vm/component/libcalls.rs)

## Questions for Alex
* Is there an effective way to zero bits on ValRaw besides lifting + lowering into zero buffer?
* How to serialize Config?
* Libcalls - what should/shouldn't need support
    * What's the difference between builtins and libcalls

### Backlog
* Benchmark and optimize
* Improve ReplayError messages
* Coalescing MemorySliceWrite


### Generating host function entry with `ftzz`

