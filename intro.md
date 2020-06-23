# Serverless for WebAssembly - Running waSCC Actors in AWS Lambda

### Wasm, WASI, waPC and waSCC

[_WebAssembly_ (Wasm)](https://webassembly.org/) is a binary instruction format designed as a compilation target for higher-level programming languages to execute at native speed on a wide range of hardware platforms. At runtime Wasm describes a sandboxed and memory-safe execution environment that can be embedded in both existing web browsers and non-web environment such as IoT devices, mobile applications or virtual servers in the cloud.

On the web platform, Wasm modules can call into and out of the browser's JavaScript context while for non-web embeddings the [_WebAssembly System Interface_ (WASI)](https://wasi.dev/) has emerged as the portable system interface for the WebAssembly platform. WASI enables Wasm code, mediated by a host, to reach out to low-level operating system APIs allowing the module to interact with file systems, random number generators, the system clock and the like. The WASI specification currently allows only a small subset of an operating system's functionality to be accessed from Wasm, but the specification is evolving to include networking functionality.

Until the WASM Interface Types [proposal](https://github.com/WebAssembly/interface-types/blob/master/proposals/interface-types/Explainer.md) has been adopted and implemented, a Wasm module on its own can only accept and return simple numeric types. This means that for robust function calling between the host and Wasm guest, developers must write custom code or use proprietary libraries. [_WebAssembly Procedure Calls_ (waPC)](https://github.com/wapc) is a standard and set of implementations that enables a bi-directional function call mechanism between the host and guest where either party does not need to have any knowledge of the other's memory-management strategy.

[_WebAssembly Secure Capabilities Connector_ (waSCC)](https://wascc.dev/) is a Wasm host runtime that dynamically and securely binds capability providers to Wasm modules implemented as [_actors_](https://wascc.dev/docs/concepts/actors/).

Capability providers come in two forms:
* **Portable Capabilities** are actors that are loaded as WASI modules and are restricted to functionality allowed in the WASI specification
* **Native Capabilities** are operating-system shared libraries loaded into the host process that execute outside the Wasm sandbox and have no restictions what they can and cannot do

waSCC includes a decentralized authorization system using [JSON Web Tokens (JWT)](https://jwt.io/) embedded into the actor's Wasm module. The JWT is cryptographically signed with a list of the capabilities that the actor can bind to and the waSCC host is responsible for enforcing the authorization.

### waSCC Hosts

A waSCC host is an operating system process that uses the [`wascc-host`](https://github.com/wascc/wascc-host/) crate to securely load and bind together actors and capability providers.
