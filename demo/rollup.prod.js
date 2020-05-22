import rust from "@wasm-tool/rollup-plugin-rust";

export default {
    input: {
        index: "./Cargo.toml",
    },
    output: {
        dir: "public/wasm/",
        format: "iife",
        sourcemap: true,
    },
    plugins: [
        rust({
            serverPath: "/wasm/",
            debug: false,
        }),
    ],
};
