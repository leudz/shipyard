# Bunny Demo

This example uses raw webgl to display lots of bunnies on WASM.
[Play Online](https://leudz.github.io/shipyard/bunny_demo)

### Build

This demo uses a JS packet manager to build and run.
You can use the one you prefer, I'll use `npm`.
All commands are made inside the `bunny_demo` directory.

Before any other command (only needed once):

```shell
npm install
```

Starts the demo in watch mode.
Changing files in `bunny_demo` will cause a re-compile and reload of the page.
It runs slower but is faster to build.

```shell
npm start
```

Build the demo in release mode, all files will be in the `public` directory.
I'm using [`microserver`](https://crates.io/crates/microserver) to make the server.

```shell
npm run build
microserver public
```

The demo is now accessible at `http://localhost:9090/` (the port might be different if you are using a different tool to make the server).
