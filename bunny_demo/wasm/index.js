(function () {
    'use strict';

    let wasm;

    const heap = new Array(32).fill(undefined);

    heap.push(undefined, null, true, false);

    function getObject(idx) { return heap[idx]; }

    let heap_next = heap.length;

    function dropObject(idx) {
        if (idx < 36) return;
        heap[idx] = heap_next;
        heap_next = idx;
    }

    function takeObject(idx) {
        const ret = getObject(idx);
        dropObject(idx);
        return ret;
    }

    function addHeapObject(obj) {
        if (heap_next === heap.length) heap.push(heap.length + 1);
        const idx = heap_next;
        heap_next = heap[idx];

        heap[idx] = obj;
        return idx;
    }

    let cachedTextDecoder = new TextDecoder('utf-8', { ignoreBOM: true, fatal: true });

    cachedTextDecoder.decode();

    let cachegetUint8Memory0 = null;
    function getUint8Memory0() {
        if (cachegetUint8Memory0 === null || cachegetUint8Memory0.buffer !== wasm.memory.buffer) {
            cachegetUint8Memory0 = new Uint8Array(wasm.memory.buffer);
        }
        return cachegetUint8Memory0;
    }

    function getStringFromWasm0(ptr, len) {
        return cachedTextDecoder.decode(getUint8Memory0().subarray(ptr, ptr + len));
    }

    function isLikeNone(x) {
        return x === undefined || x === null;
    }

    let cachegetFloat64Memory0 = null;
    function getFloat64Memory0() {
        if (cachegetFloat64Memory0 === null || cachegetFloat64Memory0.buffer !== wasm.memory.buffer) {
            cachegetFloat64Memory0 = new Float64Array(wasm.memory.buffer);
        }
        return cachegetFloat64Memory0;
    }

    let cachegetInt32Memory0 = null;
    function getInt32Memory0() {
        if (cachegetInt32Memory0 === null || cachegetInt32Memory0.buffer !== wasm.memory.buffer) {
            cachegetInt32Memory0 = new Int32Array(wasm.memory.buffer);
        }
        return cachegetInt32Memory0;
    }

    let WASM_VECTOR_LEN = 0;

    let cachedTextEncoder = new TextEncoder('utf-8');

    const encodeString = (typeof cachedTextEncoder.encodeInto === 'function'
        ? function (arg, view) {
        return cachedTextEncoder.encodeInto(arg, view);
    }
        : function (arg, view) {
        const buf = cachedTextEncoder.encode(arg);
        view.set(buf);
        return {
            read: arg.length,
            written: buf.length
        };
    });

    function passStringToWasm0(arg, malloc, realloc) {

        if (realloc === undefined) {
            const buf = cachedTextEncoder.encode(arg);
            const ptr = malloc(buf.length);
            getUint8Memory0().subarray(ptr, ptr + buf.length).set(buf);
            WASM_VECTOR_LEN = buf.length;
            return ptr;
        }

        let len = arg.length;
        let ptr = malloc(len);

        const mem = getUint8Memory0();

        let offset = 0;

        for (; offset < len; offset++) {
            const code = arg.charCodeAt(offset);
            if (code > 0x7F) break;
            mem[ptr + offset] = code;
        }

        if (offset !== len) {
            if (offset !== 0) {
                arg = arg.slice(offset);
            }
            ptr = realloc(ptr, len, len = offset + arg.length * 3);
            const view = getUint8Memory0().subarray(ptr + offset, ptr + len);
            const ret = encodeString(arg, view);

            offset += ret.written;
        }

        WASM_VECTOR_LEN = offset;
        return ptr;
    }

    function debugString(val) {
        // primitive types
        const type = typeof val;
        if (type == 'number' || type == 'boolean' || val == null) {
            return  `${val}`;
        }
        if (type == 'string') {
            return `"${val}"`;
        }
        if (type == 'symbol') {
            const description = val.description;
            if (description == null) {
                return 'Symbol';
            } else {
                return `Symbol(${description})`;
            }
        }
        if (type == 'function') {
            const name = val.name;
            if (typeof name == 'string' && name.length > 0) {
                return `Function(${name})`;
            } else {
                return 'Function';
            }
        }
        // objects
        if (Array.isArray(val)) {
            const length = val.length;
            let debug = '[';
            if (length > 0) {
                debug += debugString(val[0]);
            }
            for(let i = 1; i < length; i++) {
                debug += ', ' + debugString(val[i]);
            }
            debug += ']';
            return debug;
        }
        // Test for built-in
        const builtInMatches = /\[object ([^\]]+)\]/.exec(toString.call(val));
        let className;
        if (builtInMatches.length > 1) {
            className = builtInMatches[1];
        } else {
            // Failed to match the standard '[object ClassName]'
            return toString.call(val);
        }
        if (className == 'Object') {
            // we're a user defined class or Object
            // JSON.stringify avoids problems with cycles, and is generally much
            // easier than looping through ownProperties of `val`.
            try {
                return 'Object(' + JSON.stringify(val) + ')';
            } catch (_) {
                return 'Object';
            }
        }
        // errors
        if (val instanceof Error) {
            return `${val.name}: ${val.message}\n${val.stack}`;
        }
        // TODO we could test for more things here, like `Set`s and `Map`s.
        return className;
    }

    function makeMutClosure(arg0, arg1, dtor, f) {
        const state = { a: arg0, b: arg1, cnt: 1, dtor };
        const real = (...args) => {
            // First up with a closure we increment the internal reference
            // count. This ensures that the Rust closure environment won't
            // be deallocated while we're invoking it.
            state.cnt++;
            const a = state.a;
            state.a = 0;
            try {
                return f(a, state.b, ...args);
            } finally {
                if (--state.cnt === 0) {
                    wasm.__wbindgen_export_2.get(state.dtor)(a, state.b);

                } else {
                    state.a = a;
                }
            }
        };
        real.original = state;

        return real;
    }

    let stack_pointer = 32;

    function addBorrowedObject(obj) {
        if (stack_pointer == 1) throw new Error('out of js stack');
        heap[--stack_pointer] = obj;
        return stack_pointer;
    }
    function __wbg_adapter_28(arg0, arg1, arg2) {
        try {
            wasm._dyn_core__ops__function__FnMut___A____Output___R_as_wasm_bindgen__closure__WasmClosure___describe__invoke__hb7be3632c0ac6c03(arg0, arg1, addBorrowedObject(arg2));
        } finally {
            heap[stack_pointer++] = undefined;
        }
    }

    function __wbg_adapter_31(arg0, arg1, arg2) {
        wasm._dyn_core__ops__function__FnMut__A____Output___R_as_wasm_bindgen__closure__WasmClosure___describe__invoke__h8cd7e239df7afd49(arg0, arg1, arg2);
    }

    function __wbg_adapter_34(arg0, arg1, arg2) {
        wasm._dyn_core__ops__function__FnMut__A____Output___R_as_wasm_bindgen__closure__WasmClosure___describe__invoke__hd5d3f52f35988022(arg0, arg1, addHeapObject(arg2));
    }

    function __wbg_adapter_37(arg0, arg1) {
        wasm._dyn_core__ops__function__FnMut_____Output___R_as_wasm_bindgen__closure__WasmClosure___describe__invoke__h963ca216369628b5(arg0, arg1);
    }

    function __wbg_adapter_40(arg0, arg1, arg2) {
        wasm._dyn_core__ops__function__FnMut__A____Output___R_as_wasm_bindgen__closure__WasmClosure___describe__invoke__h319d318cc67120d2(arg0, arg1, addHeapObject(arg2));
    }

    function handleError(f, args) {
        try {
            return f.apply(this, args);
        } catch (e) {
            wasm.__wbindgen_exn_store(addHeapObject(e));
        }
    }

    let cachegetFloat32Memory0 = null;
    function getFloat32Memory0() {
        if (cachegetFloat32Memory0 === null || cachegetFloat32Memory0.buffer !== wasm.memory.buffer) {
            cachegetFloat32Memory0 = new Float32Array(wasm.memory.buffer);
        }
        return cachegetFloat32Memory0;
    }

    function getArrayF32FromWasm0(ptr, len) {
        return getFloat32Memory0().subarray(ptr / 4, ptr / 4 + len);
    }

    function getArrayU8FromWasm0(ptr, len) {
        return getUint8Memory0().subarray(ptr / 1, ptr / 1 + len);
    }

    async function load(module, imports) {
        if (typeof Response === 'function' && module instanceof Response) {
            if (typeof WebAssembly.instantiateStreaming === 'function') {
                try {
                    return await WebAssembly.instantiateStreaming(module, imports);

                } catch (e) {
                    if (module.headers.get('Content-Type') != 'application/wasm') {
                        console.warn("`WebAssembly.instantiateStreaming` failed because your server does not serve wasm with `application/wasm` MIME type. Falling back to `WebAssembly.instantiate` which is slower. Original error:\n", e);

                    } else {
                        throw e;
                    }
                }
            }

            const bytes = await module.arrayBuffer();
            return await WebAssembly.instantiate(bytes, imports);

        } else {
            const instance = await WebAssembly.instantiate(module, imports);

            if (instance instanceof WebAssembly.Instance) {
                return { instance, module };

            } else {
                return instance;
            }
        }
    }

    async function init(input) {
        if (typeof input === 'undefined') {
            input = new URL('index_bg.wasm', (document.currentScript && document.currentScript.src || new URL('index.js', document.baseURI).href));
        }
        const imports = {};
        imports.wbg = {};
        imports.wbg.__wbindgen_object_drop_ref = function(arg0) {
            takeObject(arg0);
        };
        imports.wbg.__wbindgen_cb_drop = function(arg0) {
            const obj = takeObject(arg0).original;
            if (obj.cnt-- == 1) {
                obj.a = 0;
                return true;
            }
            var ret = false;
            return ret;
        };
        imports.wbg.__wbindgen_object_clone_ref = function(arg0) {
            var ret = getObject(arg0);
            return addHeapObject(ret);
        };
        imports.wbg.__wbindgen_string_new = function(arg0, arg1) {
            var ret = getStringFromWasm0(arg0, arg1);
            return addHeapObject(ret);
        };
        imports.wbg.__wbg_new_68adb0d58759a4ed = function() {
            var ret = new Object();
            return addHeapObject(ret);
        };
        imports.wbg.__wbg_set_2e79e744454afade = function(arg0, arg1, arg2) {
            getObject(arg0)[takeObject(arg1)] = takeObject(arg2);
        };
        imports.wbg.__wbg_instanceof_Window_11e25482011fc506 = function(arg0) {
            var ret = getObject(arg0) instanceof Window;
            return ret;
        };
        imports.wbg.__wbg_document_5aff8cd83ef968f5 = function(arg0) {
            var ret = getObject(arg0).document;
            return isLikeNone(ret) ? 0 : addHeapObject(ret);
        };
        imports.wbg.__wbg_location_05eee59b82ccc208 = function(arg0) {
            var ret = getObject(arg0).location;
            return addHeapObject(ret);
        };
        imports.wbg.__wbg_innerWidth_8c5001da2fdd6a9e = function() { return handleError(function (arg0) {
            var ret = getObject(arg0).innerWidth;
            return addHeapObject(ret);
        }, arguments) };
        imports.wbg.__wbg_innerHeight_03d3f1d9eb5f7034 = function() { return handleError(function (arg0) {
            var ret = getObject(arg0).innerHeight;
            return addHeapObject(ret);
        }, arguments) };
        imports.wbg.__wbg_performance_9d1ecf711183e1d5 = function(arg0) {
            var ret = getObject(arg0).performance;
            return isLikeNone(ret) ? 0 : addHeapObject(ret);
        };
        imports.wbg.__wbg_requestAnimationFrame_1fb079d39e1b8a26 = function() { return handleError(function (arg0, arg1) {
            var ret = getObject(arg0).requestAnimationFrame(getObject(arg1));
            return ret;
        }, arguments) };
        imports.wbg.__wbg_fetch_eb9fd115eef29d0c = function(arg0, arg1) {
            var ret = getObject(arg0).fetch(getObject(arg1));
            return addHeapObject(ret);
        };
        imports.wbg.__wbg_origin_2d73a52d3c089b4a = function() { return handleError(function (arg0, arg1) {
            var ret = getObject(arg1).origin;
            var ptr0 = passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            var len0 = WASM_VECTOR_LEN;
            getInt32Memory0()[arg0 / 4 + 1] = len0;
            getInt32Memory0()[arg0 / 4 + 0] = ptr0;
        }, arguments) };
        imports.wbg.__wbg_instanceof_Response_d61ff4c524b8dbc4 = function(arg0) {
            var ret = getObject(arg0) instanceof Response;
            return ret;
        };
        imports.wbg.__wbg_text_7c3304aebfcffa1a = function() { return handleError(function (arg0) {
            var ret = getObject(arg0).text();
            return addHeapObject(ret);
        }, arguments) };
        imports.wbg.__wbg_origin_6c62658aa14705d9 = function(arg0, arg1) {
            var ret = getObject(arg1).origin;
            var ptr0 = passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            var len0 = WASM_VECTOR_LEN;
            getInt32Memory0()[arg0 / 4 + 1] = len0;
            getInt32Memory0()[arg0 / 4 + 0] = ptr0;
        };
        imports.wbg.__wbg_new_f55f190117032894 = function() { return handleError(function (arg0, arg1) {
            var ret = new URL(getStringFromWasm0(arg0, arg1));
            return addHeapObject(ret);
        }, arguments) };
        imports.wbg.__wbg_now_44a034aa2e1d73dd = function(arg0) {
            var ret = getObject(arg0).now();
            return ret;
        };
        imports.wbg.__wbg_drawArraysInstancedANGLE_298bf3c72c36e94c = function(arg0, arg1, arg2, arg3, arg4) {
            getObject(arg0).drawArraysInstancedANGLE(arg1 >>> 0, arg2, arg3, arg4);
        };
        imports.wbg.__wbg_vertexAttribDivisorANGLE_18316da6fe9d63a6 = function(arg0, arg1, arg2) {
            getObject(arg0).vertexAttribDivisorANGLE(arg1 >>> 0, arg2 >>> 0);
        };
        imports.wbg.__wbg_addEventListener_6d9a78a5d277bdaf = function() { return handleError(function (arg0, arg1, arg2, arg3, arg4) {
            getObject(arg0).addEventListener(getStringFromWasm0(arg1, arg2), getObject(arg3), getObject(arg4));
        }, arguments) };
        imports.wbg.__wbg_instanceof_HtmlCanvasElement_fd3cbbe3906d7792 = function(arg0) {
            var ret = getObject(arg0) instanceof HTMLCanvasElement;
            return ret;
        };
        imports.wbg.__wbg_width_9eb2c66ac9dde633 = function(arg0) {
            var ret = getObject(arg0).width;
            return ret;
        };
        imports.wbg.__wbg_setwidth_f3c88eb520ba8d47 = function(arg0, arg1) {
            getObject(arg0).width = arg1 >>> 0;
        };
        imports.wbg.__wbg_height_64e5d4222af3fb90 = function(arg0) {
            var ret = getObject(arg0).height;
            return ret;
        };
        imports.wbg.__wbg_setheight_5a1abba41e35c42a = function(arg0, arg1) {
            getObject(arg0).height = arg1 >>> 0;
        };
        imports.wbg.__wbg_getContext_813df131fcbd6e91 = function() { return handleError(function (arg0, arg1, arg2) {
            var ret = getObject(arg0).getContext(getStringFromWasm0(arg1, arg2));
            return isLikeNone(ret) ? 0 : addHeapObject(ret);
        }, arguments) };
        imports.wbg.__wbg_getContext_b6f46c995f9563a1 = function() { return handleError(function (arg0, arg1, arg2, arg3) {
            var ret = getObject(arg0).getContext(getStringFromWasm0(arg1, arg2), getObject(arg3));
            return isLikeNone(ret) ? 0 : addHeapObject(ret);
        }, arguments) };
        imports.wbg.__wbg_setsrc_be485ebb2fd85e29 = function(arg0, arg1, arg2) {
            getObject(arg0).src = getStringFromWasm0(arg1, arg2);
        };
        imports.wbg.__wbg_setcrossOrigin_f54c399516fd458e = function(arg0, arg1, arg2) {
            getObject(arg0).crossOrigin = arg1 === 0 ? undefined : getStringFromWasm0(arg1, arg2);
        };
        imports.wbg.__wbg_width_f23400210e588ee9 = function(arg0) {
            var ret = getObject(arg0).width;
            return ret;
        };
        imports.wbg.__wbg_height_952a1eacf8cf9513 = function(arg0) {
            var ret = getObject(arg0).height;
            return ret;
        };
        imports.wbg.__wbg_new_ed41e411ebd59bd4 = function() { return handleError(function () {
            var ret = new Image();
            return addHeapObject(ret);
        }, arguments) };
        imports.wbg.__wbg_width_5c37496c7c69eaa2 = function(arg0) {
            var ret = getObject(arg0).width;
            return ret;
        };
        imports.wbg.__wbg_height_3711225374206b37 = function(arg0) {
            var ret = getObject(arg0).height;
            return ret;
        };
        imports.wbg.__wbg_newwithstr_07dc8adf8bcc4e86 = function() { return handleError(function (arg0, arg1) {
            var ret = new Request(getStringFromWasm0(arg0, arg1));
            return addHeapObject(ret);
        }, arguments) };
        imports.wbg.__wbg_body_525168d9e773c3f8 = function(arg0) {
            var ret = getObject(arg0).body;
            return isLikeNone(ret) ? 0 : addHeapObject(ret);
        };
        imports.wbg.__wbg_createElement_ac65a6ce60c4812c = function() { return handleError(function (arg0, arg1, arg2) {
            var ret = getObject(arg0).createElement(getStringFromWasm0(arg1, arg2));
            return addHeapObject(ret);
        }, arguments) };
        imports.wbg.__wbg_instanceof_WebGlRenderingContext_c86a7d34366b6a22 = function(arg0) {
            var ret = getObject(arg0) instanceof WebGLRenderingContext;
            return ret;
        };
        imports.wbg.__wbg_canvas_f66dbbe4d24b7a9c = function(arg0) {
            var ret = getObject(arg0).canvas;
            return isLikeNone(ret) ? 0 : addHeapObject(ret);
        };
        imports.wbg.__wbg_drawingBufferWidth_251cf4c11b8fe3b7 = function(arg0) {
            var ret = getObject(arg0).drawingBufferWidth;
            return ret;
        };
        imports.wbg.__wbg_drawingBufferHeight_b65221325b738d84 = function(arg0) {
            var ret = getObject(arg0).drawingBufferHeight;
            return ret;
        };
        imports.wbg.__wbg_bufferData_fc1c7f6f7937aa2f = function(arg0, arg1, arg2, arg3) {
            getObject(arg0).bufferData(arg1 >>> 0, getObject(arg2), arg3 >>> 0);
        };
        imports.wbg.__wbg_texImage2D_c3b3326e5afec2cb = function() { return handleError(function (arg0, arg1, arg2, arg3, arg4, arg5, arg6, arg7, arg8, arg9) {
            getObject(arg0).texImage2D(arg1 >>> 0, arg2, arg3, arg4, arg5, arg6, arg7 >>> 0, arg8 >>> 0, getObject(arg9));
        }, arguments) };
        imports.wbg.__wbg_texImage2D_323a4df106591ea3 = function() { return handleError(function (arg0, arg1, arg2, arg3, arg4, arg5, arg6) {
            getObject(arg0).texImage2D(arg1 >>> 0, arg2, arg3, arg4 >>> 0, arg5 >>> 0, getObject(arg6));
        }, arguments) };
        imports.wbg.__wbg_texImage2D_0775497c58fefd11 = function() { return handleError(function (arg0, arg1, arg2, arg3, arg4, arg5, arg6) {
            getObject(arg0).texImage2D(arg1 >>> 0, arg2, arg3, arg4 >>> 0, arg5 >>> 0, getObject(arg6));
        }, arguments) };
        imports.wbg.__wbg_texImage2D_219e1a3934be9c10 = function() { return handleError(function (arg0, arg1, arg2, arg3, arg4, arg5, arg6) {
            getObject(arg0).texImage2D(arg1 >>> 0, arg2, arg3, arg4 >>> 0, arg5 >>> 0, getObject(arg6));
        }, arguments) };
        imports.wbg.__wbg_texImage2D_06913541ac737fac = function() { return handleError(function (arg0, arg1, arg2, arg3, arg4, arg5, arg6) {
            getObject(arg0).texImage2D(arg1 >>> 0, arg2, arg3, arg4 >>> 0, arg5 >>> 0, getObject(arg6));
        }, arguments) };
        imports.wbg.__wbg_texImage2D_89ebdd601880876a = function() { return handleError(function (arg0, arg1, arg2, arg3, arg4, arg5, arg6) {
            getObject(arg0).texImage2D(arg1 >>> 0, arg2, arg3, arg4 >>> 0, arg5 >>> 0, getObject(arg6));
        }, arguments) };
        imports.wbg.__wbg_uniform1fv_70832960908346ce = function(arg0, arg1, arg2, arg3) {
            getObject(arg0).uniform1fv(getObject(arg1), getArrayF32FromWasm0(arg2, arg3));
        };
        imports.wbg.__wbg_uniform2fv_8cde7153f41abfa4 = function(arg0, arg1, arg2, arg3) {
            getObject(arg0).uniform2fv(getObject(arg1), getArrayF32FromWasm0(arg2, arg3));
        };
        imports.wbg.__wbg_uniform3fv_dcb6f5e3653afca8 = function(arg0, arg1, arg2, arg3) {
            getObject(arg0).uniform3fv(getObject(arg1), getArrayF32FromWasm0(arg2, arg3));
        };
        imports.wbg.__wbg_uniform4fv_b72412f7e6b28c13 = function(arg0, arg1, arg2, arg3) {
            getObject(arg0).uniform4fv(getObject(arg1), getArrayF32FromWasm0(arg2, arg3));
        };
        imports.wbg.__wbg_uniformMatrix2fv_5c0688248e93337c = function(arg0, arg1, arg2, arg3, arg4) {
            getObject(arg0).uniformMatrix2fv(getObject(arg1), arg2 !== 0, getArrayF32FromWasm0(arg3, arg4));
        };
        imports.wbg.__wbg_uniformMatrix3fv_477c62c71a6531cf = function(arg0, arg1, arg2, arg3, arg4) {
            getObject(arg0).uniformMatrix3fv(getObject(arg1), arg2 !== 0, getArrayF32FromWasm0(arg3, arg4));
        };
        imports.wbg.__wbg_uniformMatrix4fv_ec627ec788c1b744 = function(arg0, arg1, arg2, arg3, arg4) {
            getObject(arg0).uniformMatrix4fv(getObject(arg1), arg2 !== 0, getArrayF32FromWasm0(arg3, arg4));
        };
        imports.wbg.__wbg_activeTexture_e014ee7b74cc1fca = function(arg0, arg1) {
            getObject(arg0).activeTexture(arg1 >>> 0);
        };
        imports.wbg.__wbg_attachShader_6124f72095cdcf11 = function(arg0, arg1, arg2) {
            getObject(arg0).attachShader(getObject(arg1), getObject(arg2));
        };
        imports.wbg.__wbg_bindAttribLocation_39ae178ec51863ee = function(arg0, arg1, arg2, arg3, arg4) {
            getObject(arg0).bindAttribLocation(getObject(arg1), arg2 >>> 0, getStringFromWasm0(arg3, arg4));
        };
        imports.wbg.__wbg_bindBuffer_275d909129fba2de = function(arg0, arg1, arg2) {
            getObject(arg0).bindBuffer(arg1 >>> 0, getObject(arg2));
        };
        imports.wbg.__wbg_bindTexture_f00c4b7db89d6a11 = function(arg0, arg1, arg2) {
            getObject(arg0).bindTexture(arg1 >>> 0, getObject(arg2));
        };
        imports.wbg.__wbg_blendFunc_75a5af348aa4b099 = function(arg0, arg1, arg2) {
            getObject(arg0).blendFunc(arg1 >>> 0, arg2 >>> 0);
        };
        imports.wbg.__wbg_clear_65a182ed82b4f282 = function(arg0, arg1) {
            getObject(arg0).clear(arg1 >>> 0);
        };
        imports.wbg.__wbg_clearColor_e0034d2b65202787 = function(arg0, arg1, arg2, arg3, arg4) {
            getObject(arg0).clearColor(arg1, arg2, arg3, arg4);
        };
        imports.wbg.__wbg_compileShader_42fdaee532cdb8e4 = function(arg0, arg1) {
            getObject(arg0).compileShader(getObject(arg1));
        };
        imports.wbg.__wbg_createBuffer_3691dcedc890b4e8 = function(arg0) {
            var ret = getObject(arg0).createBuffer();
            return isLikeNone(ret) ? 0 : addHeapObject(ret);
        };
        imports.wbg.__wbg_createProgram_8edfd62e0586640d = function(arg0) {
            var ret = getObject(arg0).createProgram();
            return isLikeNone(ret) ? 0 : addHeapObject(ret);
        };
        imports.wbg.__wbg_createShader_7033c38612c5688d = function(arg0, arg1) {
            var ret = getObject(arg0).createShader(arg1 >>> 0);
            return isLikeNone(ret) ? 0 : addHeapObject(ret);
        };
        imports.wbg.__wbg_createTexture_65cc306909332417 = function(arg0) {
            var ret = getObject(arg0).createTexture();
            return isLikeNone(ret) ? 0 : addHeapObject(ret);
        };
        imports.wbg.__wbg_deleteProgram_b40dd1f5e57c9978 = function(arg0, arg1) {
            getObject(arg0).deleteProgram(getObject(arg1));
        };
        imports.wbg.__wbg_deleteShader_48c95a556ad55fac = function(arg0, arg1) {
            getObject(arg0).deleteShader(getObject(arg1));
        };
        imports.wbg.__wbg_detachShader_bf6087e43eace478 = function(arg0, arg1, arg2) {
            getObject(arg0).detachShader(getObject(arg1), getObject(arg2));
        };
        imports.wbg.__wbg_disable_cb4b0073c4406d0d = function(arg0, arg1) {
            getObject(arg0).disable(arg1 >>> 0);
        };
        imports.wbg.__wbg_enable_8f92d01df1a4c77c = function(arg0, arg1) {
            getObject(arg0).enable(arg1 >>> 0);
        };
        imports.wbg.__wbg_enableVertexAttribArray_4b127e0ecccd536c = function(arg0, arg1) {
            getObject(arg0).enableVertexAttribArray(arg1 >>> 0);
        };
        imports.wbg.__wbg_generateMipmap_df35e7716b224153 = function(arg0, arg1) {
            getObject(arg0).generateMipmap(arg1 >>> 0);
        };
        imports.wbg.__wbg_getActiveAttrib_9a88c084b352612f = function(arg0, arg1, arg2) {
            var ret = getObject(arg0).getActiveAttrib(getObject(arg1), arg2 >>> 0);
            return isLikeNone(ret) ? 0 : addHeapObject(ret);
        };
        imports.wbg.__wbg_getActiveUniform_c2eb5efc60840da1 = function(arg0, arg1, arg2) {
            var ret = getObject(arg0).getActiveUniform(getObject(arg1), arg2 >>> 0);
            return isLikeNone(ret) ? 0 : addHeapObject(ret);
        };
        imports.wbg.__wbg_getAttribLocation_e3a341ce3579c6e6 = function(arg0, arg1, arg2, arg3) {
            var ret = getObject(arg0).getAttribLocation(getObject(arg1), getStringFromWasm0(arg2, arg3));
            return ret;
        };
        imports.wbg.__wbg_getExtension_adbea5bb34c458b0 = function() { return handleError(function (arg0, arg1, arg2) {
            var ret = getObject(arg0).getExtension(getStringFromWasm0(arg1, arg2));
            return isLikeNone(ret) ? 0 : addHeapObject(ret);
        }, arguments) };
        imports.wbg.__wbg_getParameter_0b14f998f44dc570 = function() { return handleError(function (arg0, arg1) {
            var ret = getObject(arg0).getParameter(arg1 >>> 0);
            return addHeapObject(ret);
        }, arguments) };
        imports.wbg.__wbg_getProgramInfoLog_0b10742df7a2ebea = function(arg0, arg1, arg2) {
            var ret = getObject(arg1).getProgramInfoLog(getObject(arg2));
            var ptr0 = isLikeNone(ret) ? 0 : passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            var len0 = WASM_VECTOR_LEN;
            getInt32Memory0()[arg0 / 4 + 1] = len0;
            getInt32Memory0()[arg0 / 4 + 0] = ptr0;
        };
        imports.wbg.__wbg_getProgramParameter_bb277a1d000dd7a1 = function(arg0, arg1, arg2) {
            var ret = getObject(arg0).getProgramParameter(getObject(arg1), arg2 >>> 0);
            return addHeapObject(ret);
        };
        imports.wbg.__wbg_getShaderInfoLog_950ab0c3fc7afa37 = function(arg0, arg1, arg2) {
            var ret = getObject(arg1).getShaderInfoLog(getObject(arg2));
            var ptr0 = isLikeNone(ret) ? 0 : passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            var len0 = WASM_VECTOR_LEN;
            getInt32Memory0()[arg0 / 4 + 1] = len0;
            getInt32Memory0()[arg0 / 4 + 0] = ptr0;
        };
        imports.wbg.__wbg_getShaderParameter_54891c5545a79869 = function(arg0, arg1, arg2) {
            var ret = getObject(arg0).getShaderParameter(getObject(arg1), arg2 >>> 0);
            return addHeapObject(ret);
        };
        imports.wbg.__wbg_getUniformLocation_8b0d07c81923dc0a = function(arg0, arg1, arg2, arg3) {
            var ret = getObject(arg0).getUniformLocation(getObject(arg1), getStringFromWasm0(arg2, arg3));
            return isLikeNone(ret) ? 0 : addHeapObject(ret);
        };
        imports.wbg.__wbg_linkProgram_0be4bd888f743eb0 = function(arg0, arg1) {
            getObject(arg0).linkProgram(getObject(arg1));
        };
        imports.wbg.__wbg_pixelStorei_e283d21924a57e2c = function(arg0, arg1, arg2) {
            getObject(arg0).pixelStorei(arg1 >>> 0, arg2);
        };
        imports.wbg.__wbg_shaderSource_c666880b620c8f34 = function(arg0, arg1, arg2, arg3) {
            getObject(arg0).shaderSource(getObject(arg1), getStringFromWasm0(arg2, arg3));
        };
        imports.wbg.__wbg_texParameteri_dd58ff2ef56511b2 = function(arg0, arg1, arg2, arg3) {
            getObject(arg0).texParameteri(arg1 >>> 0, arg2 >>> 0, arg3);
        };
        imports.wbg.__wbg_uniform1f_b57eab43144a26b1 = function(arg0, arg1, arg2) {
            getObject(arg0).uniform1f(getObject(arg1), arg2);
        };
        imports.wbg.__wbg_uniform1i_e39f64f3710aa2dc = function(arg0, arg1, arg2) {
            getObject(arg0).uniform1i(getObject(arg1), arg2);
        };
        imports.wbg.__wbg_uniform2f_82d8cb2acf928fdc = function(arg0, arg1, arg2, arg3) {
            getObject(arg0).uniform2f(getObject(arg1), arg2, arg3);
        };
        imports.wbg.__wbg_uniform3f_35b8f0f096e8197a = function(arg0, arg1, arg2, arg3, arg4) {
            getObject(arg0).uniform3f(getObject(arg1), arg2, arg3, arg4);
        };
        imports.wbg.__wbg_uniform4f_996d28a7a4fc06ae = function(arg0, arg1, arg2, arg3, arg4, arg5) {
            getObject(arg0).uniform4f(getObject(arg1), arg2, arg3, arg4, arg5);
        };
        imports.wbg.__wbg_useProgram_fb4984fb080bcd61 = function(arg0, arg1) {
            getObject(arg0).useProgram(getObject(arg1));
        };
        imports.wbg.__wbg_vertexAttribPointer_df432e7ac8f60b63 = function(arg0, arg1, arg2, arg3, arg4, arg5, arg6) {
            getObject(arg0).vertexAttribPointer(arg1 >>> 0, arg2, arg3 >>> 0, arg4 !== 0, arg5, arg6);
        };
        imports.wbg.__wbg_viewport_39356c8cdec98b8b = function(arg0, arg1, arg2, arg3, arg4) {
            getObject(arg0).viewport(arg1, arg2, arg3, arg4);
        };
        imports.wbg.__wbg_setclassName_09e9074a6eb1e2cb = function(arg0, arg1, arg2) {
            getObject(arg0).className = getStringFromWasm0(arg1, arg2);
        };
        imports.wbg.__wbg_instanceof_HtmlElement_835072e813858ac0 = function(arg0) {
            var ret = getObject(arg0) instanceof HTMLElement;
            return ret;
        };
        imports.wbg.__wbg_setonload_42a438d19db596f4 = function(arg0, arg1) {
            getObject(arg0).onload = getObject(arg1);
        };
        imports.wbg.__wbg_setonerror_61618db3d13ead14 = function(arg0, arg1) {
            getObject(arg0).onerror = getObject(arg1);
        };
        imports.wbg.__wbg_new_d110e781a7944595 = function() { return handleError(function (arg0, arg1) {
            var ret = new Event(getStringFromWasm0(arg0, arg1));
            return addHeapObject(ret);
        }, arguments) };
        imports.wbg.__wbg_width_7ab552b6053e94e1 = function(arg0) {
            var ret = getObject(arg0).width;
            return ret;
        };
        imports.wbg.__wbg_height_19cc0180695c420d = function(arg0) {
            var ret = getObject(arg0).height;
            return ret;
        };
        imports.wbg.__wbg_width_9ae660cbd519b85b = function(arg0) {
            var ret = getObject(arg0).width;
            return ret;
        };
        imports.wbg.__wbg_height_6f2ee9289e7bddd1 = function(arg0) {
            var ret = getObject(arg0).height;
            return ret;
        };
        imports.wbg.__wbg_settextContent_2e55253528a044b7 = function(arg0, arg1, arg2) {
            getObject(arg0).textContent = arg1 === 0 ? undefined : getStringFromWasm0(arg1, arg2);
        };
        imports.wbg.__wbg_appendChild_6ed236bb79c198df = function() { return handleError(function (arg0, arg1) {
            var ret = getObject(arg0).appendChild(getObject(arg1));
            return addHeapObject(ret);
        }, arguments) };
        imports.wbg.__wbg_removeChild_f633f19eb895b696 = function() { return handleError(function (arg0, arg1) {
            var ret = getObject(arg0).removeChild(getObject(arg1));
            return addHeapObject(ret);
        }, arguments) };
        imports.wbg.__wbg_size_d796992ceb41c927 = function(arg0) {
            var ret = getObject(arg0).size;
            return ret;
        };
        imports.wbg.__wbg_type_a6dbb472e3a53b1d = function(arg0) {
            var ret = getObject(arg0).type;
            return ret;
        };
        imports.wbg.__wbg_name_3bda187a3f4d705f = function(arg0, arg1) {
            var ret = getObject(arg1).name;
            var ptr0 = passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            var len0 = WASM_VECTOR_LEN;
            getInt32Memory0()[arg0 / 4 + 1] = len0;
            getInt32Memory0()[arg0 / 4 + 0] = ptr0;
        };
        imports.wbg.__wbg_getRandomValues_98117e9a7e993920 = function() { return handleError(function (arg0, arg1) {
            getObject(arg0).getRandomValues(getObject(arg1));
        }, arguments) };
        imports.wbg.__wbg_randomFillSync_64cc7d048f228ca8 = function() { return handleError(function (arg0, arg1, arg2) {
            getObject(arg0).randomFillSync(getArrayU8FromWasm0(arg1, arg2));
        }, arguments) };
        imports.wbg.__wbg_process_2f24d6544ea7b200 = function(arg0) {
            var ret = getObject(arg0).process;
            return addHeapObject(ret);
        };
        imports.wbg.__wbindgen_is_object = function(arg0) {
            const val = getObject(arg0);
            var ret = typeof(val) === 'object' && val !== null;
            return ret;
        };
        imports.wbg.__wbg_versions_6164651e75405d4a = function(arg0) {
            var ret = getObject(arg0).versions;
            return addHeapObject(ret);
        };
        imports.wbg.__wbg_node_4b517d861cbcb3bc = function(arg0) {
            var ret = getObject(arg0).node;
            return addHeapObject(ret);
        };
        imports.wbg.__wbg_modulerequire_3440a4bcf44437db = function() { return handleError(function (arg0, arg1) {
            var ret = module.require(getStringFromWasm0(arg0, arg1));
            return addHeapObject(ret);
        }, arguments) };
        imports.wbg.__wbg_crypto_98fc271021c7d2ad = function(arg0) {
            var ret = getObject(arg0).crypto;
            return addHeapObject(ret);
        };
        imports.wbg.__wbg_msCrypto_a2cdb043d2bfe57f = function(arg0) {
            var ret = getObject(arg0).msCrypto;
            return addHeapObject(ret);
        };
        imports.wbg.__wbg_call_ba36642bd901572b = function() { return handleError(function (arg0, arg1) {
            var ret = getObject(arg0).call(getObject(arg1));
            return addHeapObject(ret);
        }, arguments) };
        imports.wbg.__wbg_newnoargs_9fdd8f3961dd1bee = function(arg0, arg1) {
            var ret = new Function(getStringFromWasm0(arg0, arg1));
            return addHeapObject(ret);
        };
        imports.wbg.__wbg_new_edbe38a4e21329dd = function() {
            var ret = new Object();
            return addHeapObject(ret);
        };
        imports.wbg.__wbg_resolve_cae3d8f752f5db88 = function(arg0) {
            var ret = Promise.resolve(getObject(arg0));
            return addHeapObject(ret);
        };
        imports.wbg.__wbg_then_c2361a9d5c9a4fcb = function(arg0, arg1) {
            var ret = getObject(arg0).then(getObject(arg1));
            return addHeapObject(ret);
        };
        imports.wbg.__wbg_then_6c9a4bf55755f9b8 = function(arg0, arg1, arg2) {
            var ret = getObject(arg0).then(getObject(arg1), getObject(arg2));
            return addHeapObject(ret);
        };
        imports.wbg.__wbg_self_bb69a836a72ec6e9 = function() { return handleError(function () {
            var ret = self.self;
            return addHeapObject(ret);
        }, arguments) };
        imports.wbg.__wbg_window_3304fc4b414c9693 = function() { return handleError(function () {
            var ret = window.window;
            return addHeapObject(ret);
        }, arguments) };
        imports.wbg.__wbg_globalThis_e0d21cabc6630763 = function() { return handleError(function () {
            var ret = globalThis.globalThis;
            return addHeapObject(ret);
        }, arguments) };
        imports.wbg.__wbg_global_8463719227271676 = function() { return handleError(function () {
            var ret = global.global;
            return addHeapObject(ret);
        }, arguments) };
        imports.wbg.__wbindgen_is_undefined = function(arg0) {
            var ret = getObject(arg0) === undefined;
            return ret;
        };
        imports.wbg.__wbg_buffer_9e184d6f785de5ed = function(arg0) {
            var ret = getObject(arg0).buffer;
            return addHeapObject(ret);
        };
        imports.wbg.__wbg_length_2d56cb37075fcfb1 = function(arg0) {
            var ret = getObject(arg0).length;
            return ret;
        };
        imports.wbg.__wbg_new_e8101319e4cf95fc = function(arg0) {
            var ret = new Uint8Array(getObject(arg0));
            return addHeapObject(ret);
        };
        imports.wbg.__wbg_set_e8ae7b27314e8b98 = function(arg0, arg1, arg2) {
            getObject(arg0).set(getObject(arg1), arg2 >>> 0);
        };
        imports.wbg.__wbg_newwithbyteoffsetandlength_abfc24c57fd08e6d = function(arg0, arg1, arg2) {
            var ret = new Float32Array(getObject(arg0), arg1 >>> 0, arg2 >>> 0);
            return addHeapObject(ret);
        };
        imports.wbg.__wbg_length_523cf4a01041a7a6 = function(arg0) {
            var ret = getObject(arg0).length;
            return ret;
        };
        imports.wbg.__wbg_new_d69bbe3db485d457 = function(arg0) {
            var ret = new Float32Array(getObject(arg0));
            return addHeapObject(ret);
        };
        imports.wbg.__wbg_set_0fa56ca792d65861 = function(arg0, arg1, arg2) {
            getObject(arg0).set(getObject(arg1), arg2 >>> 0);
        };
        imports.wbg.__wbg_newwithlength_a8d1dbcbe703a5c6 = function(arg0) {
            var ret = new Uint8Array(arg0 >>> 0);
            return addHeapObject(ret);
        };
        imports.wbg.__wbg_subarray_901ede8318da52a6 = function(arg0, arg1, arg2) {
            var ret = getObject(arg0).subarray(arg1 >>> 0, arg2 >>> 0);
            return addHeapObject(ret);
        };
        imports.wbg.__wbg_set_73349fc4814e0fc6 = function() { return handleError(function (arg0, arg1, arg2) {
            var ret = Reflect.set(getObject(arg0), getObject(arg1), getObject(arg2));
            return ret;
        }, arguments) };
        imports.wbg.__wbindgen_number_get = function(arg0, arg1) {
            const obj = getObject(arg1);
            var ret = typeof(obj) === 'number' ? obj : undefined;
            getFloat64Memory0()[arg0 / 8 + 1] = isLikeNone(ret) ? 0 : ret;
            getInt32Memory0()[arg0 / 4 + 0] = !isLikeNone(ret);
        };
        imports.wbg.__wbindgen_is_string = function(arg0) {
            var ret = typeof(getObject(arg0)) === 'string';
            return ret;
        };
        imports.wbg.__wbindgen_string_get = function(arg0, arg1) {
            const obj = getObject(arg1);
            var ret = typeof(obj) === 'string' ? obj : undefined;
            var ptr0 = isLikeNone(ret) ? 0 : passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            var len0 = WASM_VECTOR_LEN;
            getInt32Memory0()[arg0 / 4 + 1] = len0;
            getInt32Memory0()[arg0 / 4 + 0] = ptr0;
        };
        imports.wbg.__wbindgen_boolean_get = function(arg0) {
            const v = getObject(arg0);
            var ret = typeof(v) === 'boolean' ? (v ? 1 : 0) : 2;
            return ret;
        };
        imports.wbg.__wbindgen_debug_string = function(arg0, arg1) {
            var ret = debugString(getObject(arg1));
            var ptr0 = passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            var len0 = WASM_VECTOR_LEN;
            getInt32Memory0()[arg0 / 4 + 1] = len0;
            getInt32Memory0()[arg0 / 4 + 0] = ptr0;
        };
        imports.wbg.__wbindgen_throw = function(arg0, arg1) {
            throw new Error(getStringFromWasm0(arg0, arg1));
        };
        imports.wbg.__wbindgen_memory = function() {
            var ret = wasm.memory;
            return addHeapObject(ret);
        };
        imports.wbg.__wbindgen_closure_wrapper219 = function(arg0, arg1, arg2) {
            var ret = makeMutClosure(arg0, arg1, 41, __wbg_adapter_28);
            return addHeapObject(ret);
        };
        imports.wbg.__wbindgen_closure_wrapper221 = function(arg0, arg1, arg2) {
            var ret = makeMutClosure(arg0, arg1, 41, __wbg_adapter_31);
            return addHeapObject(ret);
        };
        imports.wbg.__wbindgen_closure_wrapper570 = function(arg0, arg1, arg2) {
            var ret = makeMutClosure(arg0, arg1, 239, __wbg_adapter_34);
            return addHeapObject(ret);
        };
        imports.wbg.__wbindgen_closure_wrapper572 = function(arg0, arg1, arg2) {
            var ret = makeMutClosure(arg0, arg1, 239, __wbg_adapter_37);
            return addHeapObject(ret);
        };
        imports.wbg.__wbindgen_closure_wrapper713 = function(arg0, arg1, arg2) {
            var ret = makeMutClosure(arg0, arg1, 284, __wbg_adapter_40);
            return addHeapObject(ret);
        };

        if (typeof input === 'string' || (typeof Request === 'function' && input instanceof Request) || (typeof URL === 'function' && input instanceof URL)) {
            input = fetch(input);
        }



        const { instance, module } = await load(await input, imports);

        wasm = instance.exports;
        init.__wbindgen_wasm_module = module;
        wasm.__wbindgen_start();
        return wasm;
    }

    init("wasm/shipyard_bunny_demo.wasm").catch(console.error);

}());
//# sourceMappingURL=index.js.map
