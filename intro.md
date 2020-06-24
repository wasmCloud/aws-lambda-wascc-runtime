# Serverless WebAssembly - Running waSCC Actors in AWS Lambda

Serverless and WebAssembly, two technologies of the moment and with great promise for the future, but not often spoken of in the same breath.
This is a tale of how to run WebAssembly modules in the cloud on the AWS Lambda serverless platform. But first we'll need to fill you in on the backstory.

### Wasm, WASI, waPC and waSCC

[_WebAssembly_ (Wasm)](https://webassembly.org/) is a binary instruction format designed as a compilation target for higher-level programming languages to execute at native speed on a wide range of hardware platforms. At runtime Wasm describes a sandboxed and memory-safe execution environment that can be embedded in both existing web browsers and non-web environment such as IoT devices, mobile applications or virtual servers in the cloud.

On the web platform, Wasm modules can call into and out of the browser's JavaScript context while for non-web embeddings the [_WebAssembly System Interface_ (WASI)](https://wasi.dev/) has emerged as the portable system interface for the WebAssembly platform. WASI enables Wasm code, mediated by a host, to reach out to low-level operating system APIs allowing the module to interact with file systems, random number generators, the system clock and the like. The WASI specification currently allows only a small subset of an operating system's functionality to be accessed from Wasm, but the specification is evolving to include networking functionality.

Until the WASM Interface Types [proposal](https://github.com/WebAssembly/interface-types/blob/master/proposals/interface-types/Explainer.md) has been adopted and implemented, a Wasm module on its own can only accept and return simple numeric types. This means that for robust function calling between the host and Wasm guest, developers must write custom code or use proprietary libraries. [_WebAssembly Procedure Calls_ (waPC)](https://github.com/wapc) is a standard and set of implementations that enables a bi-directional function call mechanism between the host and guest where either party does not need to have any knowledge of the other's memory-management strategy.

[_WebAssembly Secure Capabilities Connector_ (waSCC)](https://wascc.dev/) is a Wasm host runtime, using waPC, that dynamically and securely binds capability providers to Wasm modules implemented as [_actors_](https://wascc.dev/docs/concepts/actors/).

Capability providers come in two forms:
* **Portable Capabilities** are actors that are loaded as WASI modules and are restricted to functionality allowed in the WASI specification
* **Native Capabilities** are operating-system shared libraries loaded into the host process that execute outside the Wasm sandbox and have no restictions on what they can and cannot do

waSCC includes a decentralized authorization system using [JSON Web Tokens (JWT)](https://jwt.io/) embedded into the actor's Wasm module. The JWT is cryptographically signed with a list of the capabilities that the actor can bind to and the waSCC host is responsible for enforcing the authorization during binding.

### waSCC Hosts

A waSCC host is an operating system process that uses the [`wascc-host`](https://github.com/wascc/wascc-host/) crate to securely load and bind together actors and capability providers and provides the communication channel between them.
For the Kubernetes ecosystem DeisLabs have created [Krustlet](https://deislabs.io/posts/introducing-krustlet/), the WebAssembly Kubelet, which allows WASI modules and waSCC actors to be deployed into Kubernetes clusters.

But what if you want to run Wasm in the cloud without provisioning and operating any infrastructure and only paying for the resources you actually use, not all the resources you ask for but don't use? AWS Lambda, and in-particular [custom runtimes](https://docs.aws.amazon.com/lambda/latest/dg/runtimes-custom.html) (first introduced at AWS re:Invent 2018), give us the building blocks to be able to do this.

Imagine being able to take a waSCC actor built to respond to HTTP requests in a Kubernetes cluster using Krustlet and deploying it completely unchanged as a Lambda function that runs with Amazon API Gateway REST or HTTP APIs. It's for exactly this scenario that we built the [waSCC Runtime for AWS Lambda](https://github.com/wascc/aws-lambda-wascc-runtime).

## How It Works

At the core, an AWS Lambda custom runtime is an executable file, named `bootstrap`, that runs in the standard Lambda [execution environment](https://docs.aws.amazon.com/lambda/latest/dg/lambda-runtimes.html). It's responsible for polling an HTTP endpoint exposed by that execution environment, retrieving Lambda invocation events, processing the events and replying with either an invocation response or an error.

The waSCC Runtime for AWS Lambda is a waSCC host with the polling workflow implemented as a capability provider statically linked (together with the waSCC [standard logging provider](https://github.com/wascc/logging-provider)) into a single file that can either be included at the root of the Lambda's ZIP file or deployed as a [layer](https://docs.aws.amazon.com/lambda/latest/dg/configuration-layers.html). Also in the Lambda's ZIP file are the actor modules and a YAML manifest file, `manifest.yaml`, that defines the Wasm module(s) to be loaded.

If the actor's claims include the standard `wascc:http_server` capability the runtime dispatches Application Load Balancer [target group requests](https://docs.aws.amazon.com/lambda/latest/dg/services-alb.html) and API Gateway (REST and HTTP APIs) [proxy requests](https://docs.aws.amazon.com/lambda/latest/dg/services-apigateway.html) to the actor (and converts the actor's response to the appropriate Lambda invocation response). Otherwise if the actor's claims include the custom `awslambda:event` capability the runtime dispatches the raw Lambda event to the actor.
