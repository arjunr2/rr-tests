# Component Model Architecture Clarifications

## Instances

Note the difference between the following:

1. `instance`: This is a **component-level** instance with types, exports, and funcs all referring to **component index space**. This can be instantiated with
`instantiate <component> (with <component-import-module-name>")` and refers to the 

2. `core instance`: This is a **core-level** instance, which can be:
    * Instantiated as runnable: `instantiate <core-module> (with "<core-import-module-name>" (instance <component-instance>))`
    * Synthetic for wiring: `export ""`


## Aliases

Note the difference between the following:

1. `alias export <component-instance> <name> ...`: This is a **component-level** export alias. These are things that can be accessed/called from the outside world

2. `alias core export <core-instance> <name>`: This is a **core-level** export alias that **CANNOT** be called from the outside world. The main purpose is just to add this function to the core index space of the component to call things like `canon lift` on it (which can thereafter be exported at the component-level with `export`).