
(function(l, r) { if (l.getElementById('livereloadscript')) return; r = l.createElement('script'); r.async = 1; r.src = '//' + (window.location.host || 'localhost').split(':')[0] + ':35729/livereload.js?snipver=1'; r.id = 'livereloadscript'; l.getElementsByTagName('head')[0].appendChild(r) })(window.document);
(function () {
    'use strict';

    let wasm;

    const heap = new Array(32).fill(undefined);

    heap.push(undefined, null, true, false);

    function getObject(idx) { return heap[idx]; }

    let heap_next = heap.length;

    function addHeapObject(obj) {
        if (heap_next === heap.length) heap.push(heap.length + 1);
        const idx = heap_next;
        heap_next = heap[idx];

        if (typeof(heap_next) !== 'number') throw new Error('corrupt heap');

        heap[idx] = obj;
        return idx;
    }

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

    function _assertBoolean(n) {
        if (typeof(n) !== 'boolean') {
            throw new Error('expected a boolean argument');
        }
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

    function _assertNum(n) {
        if (typeof(n) !== 'number') throw new Error('expected a number argument');
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

        if (typeof(arg) !== 'string') throw new Error('expected a string argument');

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
            if (ret.read !== arg.length) throw new Error('failed to pass whole string');
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
        const state = { a: arg0, b: arg1, cnt: 1 };
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
                if (--state.cnt === 0) wasm.__wbindgen_export_2.get(dtor)(a, state.b);
                else state.a = a;
            }
        };
        real.original = state;
        return real;
    }

    function logError(f) {
        return function () {
            try {
                return f.apply(this, arguments);

            } catch (e) {
                let error = (function () {
                    try {
                        return e instanceof Error ? `${e.message}\n\nStack:\n${e.stack}` : e.toString();
                    } catch(_) {
                        return "<failed to stringify thrown value>";
                    }
                }());
                console.error("wasm-bindgen: imported JS function that was not marked as `catch` threw an error:", error);
                throw e;
            }
        };
    }
    function __wbg_adapter_26(arg0, arg1) {
        _assertNum(arg0);
        _assertNum(arg1);
        wasm._dyn_core__ops__function__FnMut_____Output___R_as_wasm_bindgen__closure__WasmClosure___describe__invoke__hbbe2b5ea28d00457(arg0, arg1);
    }

    function __wbg_adapter_29(arg0, arg1, arg2) {
        _assertNum(arg0);
        _assertNum(arg1);
        wasm._dyn_core__ops__function__FnMut__A____Output___R_as_wasm_bindgen__closure__WasmClosure___describe__invoke__h069396bd365467b3(arg0, arg1, addHeapObject(arg2));
    }

    function __wbg_adapter_32(arg0, arg1, arg2) {
        _assertNum(arg0);
        _assertNum(arg1);
        wasm._dyn_core__ops__function__FnMut__A____Output___R_as_wasm_bindgen__closure__WasmClosure___describe__invoke__hcfc4f1cf0f85f233(arg0, arg1, arg2);
    }

    let stack_pointer = 32;

    function addBorrowedObject(obj) {
        if (stack_pointer == 1) throw new Error('out of js stack');
        heap[--stack_pointer] = obj;
        return stack_pointer;
    }
    function __wbg_adapter_35(arg0, arg1, arg2) {
        try {
            _assertNum(arg0);
            _assertNum(arg1);
            wasm._dyn_core__ops__function__FnMut___A____Output___R_as_wasm_bindgen__closure__WasmClosure___describe__invoke__h88555f69a9e24bbe(arg0, arg1, addBorrowedObject(arg2));
        } finally {
            heap[stack_pointer++] = undefined;
        }
    }

    function handleError(f) {
        return function () {
            try {
                return f.apply(this, arguments);

            } catch (e) {
                wasm.__wbindgen_exn_store(addHeapObject(e));
            }
        };
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
            input = (document.currentScript && document.currentScript.src || new URL('index.js', document.baseURI).href).replace(/\.js$/, '_bg.wasm');
        }
        const imports = {};
        imports.wbg = {};
        imports.wbg.__wbindgen_object_clone_ref = function(arg0) {
            var ret = getObject(arg0);
            return addHeapObject(ret);
        };
        imports.wbg.__wbindgen_cb_forget = function(arg0) {
            takeObject(arg0);
        };
        imports.wbg.__wbindgen_cb_drop = function(arg0) {
            const obj = takeObject(arg0).original;
            if (obj.cnt-- == 1) {
                obj.a = 0;
                return true;
            }
            var ret = false;
            _assertBoolean(ret);
            return ret;
        };
        imports.wbg.__wbindgen_string_new = function(arg0, arg1) {
            var ret = getStringFromWasm0(arg0, arg1);
            return addHeapObject(ret);
        };
        imports.wbg.__wbg_new_68adb0d58759a4ed = logError(function() {
            var ret = new Object();
            return addHeapObject(ret);
        });
        imports.wbg.__wbg_set_2e79e744454afade = logError(function(arg0, arg1, arg2) {
            getObject(arg0)[takeObject(arg1)] = takeObject(arg2);
        });
        imports.wbg.__wbindgen_is_undefined = function(arg0) {
            var ret = getObject(arg0) === undefined;
            _assertBoolean(ret);
            return ret;
        };
        imports.wbg.__wbg_instanceof_Window_17fdb5cd280d476d = logError(function(arg0) {
            var ret = getObject(arg0) instanceof Window;
            _assertBoolean(ret);
            return ret;
        });
        imports.wbg.__wbg_document_c26d0f423c143e0c = logError(function(arg0) {
            var ret = getObject(arg0).document;
            return isLikeNone(ret) ? 0 : addHeapObject(ret);
        });
        imports.wbg.__wbg_location_55774a0e1fed1144 = logError(function(arg0) {
            var ret = getObject(arg0).location;
            return addHeapObject(ret);
        });
        imports.wbg.__wbg_innerWidth_7b34256e80f42a06 = handleError(function(arg0) {
            var ret = getObject(arg0).innerWidth;
            return addHeapObject(ret);
        });
        imports.wbg.__wbg_innerHeight_91035c3a853be26f = handleError(function(arg0) {
            var ret = getObject(arg0).innerHeight;
            return addHeapObject(ret);
        });
        imports.wbg.__wbg_performance_781c00e4226de6c4 = logError(function(arg0) {
            var ret = getObject(arg0).performance;
            return isLikeNone(ret) ? 0 : addHeapObject(ret);
        });
        imports.wbg.__wbg_requestAnimationFrame_284f4f875590aa84 = handleError(function(arg0, arg1) {
            var ret = getObject(arg0).requestAnimationFrame(getObject(arg1));
            _assertNum(ret);
            return ret;
        });
        imports.wbg.__wbg_fetch_8047bcf6e8caf7db = logError(function(arg0, arg1) {
            var ret = getObject(arg0).fetch(getObject(arg1));
            return addHeapObject(ret);
        });
        imports.wbg.__wbg_origin_0b6a19efa55b906e = handleError(function(arg0, arg1) {
            var ret = getObject(arg1).origin;
            var ptr0 = passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            var len0 = WASM_VECTOR_LEN;
            getInt32Memory0()[arg0 / 4 + 1] = len0;
            getInt32Memory0()[arg0 / 4 + 0] = ptr0;
        });
        imports.wbg.__wbg_settextContent_917f10f51a06bd14 = logError(function(arg0, arg1, arg2) {
            getObject(arg0).textContent = arg1 === 0 ? undefined : getStringFromWasm0(arg1, arg2);
        });
        imports.wbg.__wbg_appendChild_3d4ec7dbf3472d31 = handleError(function(arg0, arg1) {
            var ret = getObject(arg0).appendChild(getObject(arg1));
            return addHeapObject(ret);
        });
        imports.wbg.__wbg_removeChild_d8035999cf171601 = handleError(function(arg0, arg1) {
            var ret = getObject(arg0).removeChild(getObject(arg1));
            return addHeapObject(ret);
        });
        imports.wbg.__wbg_instanceof_Response_64fe4248a574e920 = logError(function(arg0) {
            var ret = getObject(arg0) instanceof Response;
            _assertBoolean(ret);
            return ret;
        });
        imports.wbg.__wbg_text_39a4ddf8fca1ea2a = handleError(function(arg0) {
            var ret = getObject(arg0).text();
            return addHeapObject(ret);
        });
        imports.wbg.__wbg_origin_404b75ebff9bb04b = logError(function(arg0, arg1) {
            var ret = getObject(arg1).origin;
            var ptr0 = passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            var len0 = WASM_VECTOR_LEN;
            getInt32Memory0()[arg0 / 4 + 1] = len0;
            getInt32Memory0()[arg0 / 4 + 0] = ptr0;
        });
        imports.wbg.__wbg_new_3e81fd9e28244208 = handleError(function(arg0, arg1) {
            var ret = new URL(getStringFromWasm0(arg0, arg1));
            return addHeapObject(ret);
        });
        imports.wbg.__wbg_now_43100dbb52857cc6 = logError(function(arg0) {
            var ret = getObject(arg0).now();
            return ret;
        });
        imports.wbg.__wbg_drawArraysInstancedANGLE_83b03dcc82db7f1c = logError(function(arg0, arg1, arg2, arg3, arg4) {
            getObject(arg0).drawArraysInstancedANGLE(arg1 >>> 0, arg2, arg3, arg4);
        });
        imports.wbg.__wbg_vertexAttribDivisorANGLE_1302ec557b2e9ebf = logError(function(arg0, arg1, arg2) {
            getObject(arg0).vertexAttribDivisorANGLE(arg1 >>> 0, arg2 >>> 0);
        });
        imports.wbg.__wbg_addEventListener_3526086a053a131e = handleError(function(arg0, arg1, arg2, arg3, arg4) {
            getObject(arg0).addEventListener(getStringFromWasm0(arg1, arg2), getObject(arg3), getObject(arg4));
        });
        imports.wbg.__wbg_removeEventListener_003b13762a00969d = handleError(function(arg0, arg1, arg2, arg3, arg4) {
            getObject(arg0).removeEventListener(getStringFromWasm0(arg1, arg2), getObject(arg3), arg4 !== 0);
        });
        imports.wbg.__wbg_instanceof_HtmlCanvasElement_ff7be16a6a6bdf51 = logError(function(arg0) {
            var ret = getObject(arg0) instanceof HTMLCanvasElement;
            _assertBoolean(ret);
            return ret;
        });
        imports.wbg.__wbg_width_aeeb90e24b778e64 = logError(function(arg0) {
            var ret = getObject(arg0).width;
            _assertNum(ret);
            return ret;
        });
        imports.wbg.__wbg_setwidth_486e88fb4e1db26c = logError(function(arg0, arg1) {
            getObject(arg0).width = arg1 >>> 0;
        });
        imports.wbg.__wbg_height_66b10992a66b71e3 = logError(function(arg0) {
            var ret = getObject(arg0).height;
            _assertNum(ret);
            return ret;
        });
        imports.wbg.__wbg_setheight_ef6b352fbb18b65b = logError(function(arg0, arg1) {
            getObject(arg0).height = arg1 >>> 0;
        });
        imports.wbg.__wbg_getContext_0dcf09cb63d08f77 = handleError(function(arg0, arg1, arg2) {
            var ret = getObject(arg0).getContext(getStringFromWasm0(arg1, arg2));
            return isLikeNone(ret) ? 0 : addHeapObject(ret);
        });
        imports.wbg.__wbg_getContext_0f76d1447a9a7f6b = handleError(function(arg0, arg1, arg2, arg3) {
            var ret = getObject(arg0).getContext(getStringFromWasm0(arg1, arg2), getObject(arg3));
            return isLikeNone(ret) ? 0 : addHeapObject(ret);
        });
        imports.wbg.__wbg_newwithstr_b69dcf71f839f613 = handleError(function(arg0, arg1) {
            var ret = new Request(getStringFromWasm0(arg0, arg1));
            return addHeapObject(ret);
        });
        imports.wbg.__wbg_body_be181e812b4c9a18 = logError(function(arg0) {
            var ret = getObject(arg0).body;
            return isLikeNone(ret) ? 0 : addHeapObject(ret);
        });
        imports.wbg.__wbg_createElement_44ab59c4ad367831 = handleError(function(arg0, arg1, arg2) {
            var ret = getObject(arg0).createElement(getStringFromWasm0(arg1, arg2));
            return addHeapObject(ret);
        });
        imports.wbg.__wbg_setclassName_f867a8bb05e9072a = logError(function(arg0, arg1, arg2) {
            getObject(arg0).className = getStringFromWasm0(arg1, arg2);
        });
        imports.wbg.__wbg_instanceof_WebGlRenderingContext_f732dd95c50b903a = logError(function(arg0) {
            var ret = getObject(arg0) instanceof WebGLRenderingContext;
            _assertBoolean(ret);
            return ret;
        });
        imports.wbg.__wbg_canvas_fc9d9d7a8db9c8e6 = logError(function(arg0) {
            var ret = getObject(arg0).canvas;
            return isLikeNone(ret) ? 0 : addHeapObject(ret);
        });
        imports.wbg.__wbg_drawingBufferWidth_f6e2e679bef8d30c = logError(function(arg0) {
            var ret = getObject(arg0).drawingBufferWidth;
            _assertNum(ret);
            return ret;
        });
        imports.wbg.__wbg_drawingBufferHeight_53a875c847d3f27e = logError(function(arg0) {
            var ret = getObject(arg0).drawingBufferHeight;
            _assertNum(ret);
            return ret;
        });
        imports.wbg.__wbg_bufferData_a513e51e685294ae = logError(function(arg0, arg1, arg2, arg3) {
            getObject(arg0).bufferData(arg1 >>> 0, getObject(arg2), arg3 >>> 0);
        });
        imports.wbg.__wbg_texImage2D_463252029a01c306 = handleError(function(arg0, arg1, arg2, arg3, arg4, arg5, arg6, arg7, arg8, arg9) {
            getObject(arg0).texImage2D(arg1 >>> 0, arg2, arg3, arg4, arg5, arg6, arg7 >>> 0, arg8 >>> 0, getObject(arg9));
        });
        imports.wbg.__wbg_texImage2D_a77ad645f099073c = handleError(function(arg0, arg1, arg2, arg3, arg4, arg5, arg6) {
            getObject(arg0).texImage2D(arg1 >>> 0, arg2, arg3, arg4 >>> 0, arg5 >>> 0, getObject(arg6));
        });
        imports.wbg.__wbg_texImage2D_df475ff5163f8301 = handleError(function(arg0, arg1, arg2, arg3, arg4, arg5, arg6) {
            getObject(arg0).texImage2D(arg1 >>> 0, arg2, arg3, arg4 >>> 0, arg5 >>> 0, getObject(arg6));
        });
        imports.wbg.__wbg_texImage2D_63b904104a341e10 = handleError(function(arg0, arg1, arg2, arg3, arg4, arg5, arg6) {
            getObject(arg0).texImage2D(arg1 >>> 0, arg2, arg3, arg4 >>> 0, arg5 >>> 0, getObject(arg6));
        });
        imports.wbg.__wbg_texImage2D_5798050878f7ee6e = handleError(function(arg0, arg1, arg2, arg3, arg4, arg5, arg6) {
            getObject(arg0).texImage2D(arg1 >>> 0, arg2, arg3, arg4 >>> 0, arg5 >>> 0, getObject(arg6));
        });
        imports.wbg.__wbg_texImage2D_fcb01de38ce5a408 = handleError(function(arg0, arg1, arg2, arg3, arg4, arg5, arg6) {
            getObject(arg0).texImage2D(arg1 >>> 0, arg2, arg3, arg4 >>> 0, arg5 >>> 0, getObject(arg6));
        });
        imports.wbg.__wbg_uniform1fv_0d4578db3d893098 = logError(function(arg0, arg1, arg2, arg3) {
            getObject(arg0).uniform1fv(getObject(arg1), getArrayF32FromWasm0(arg2, arg3));
        });
        imports.wbg.__wbg_uniform2fv_a32f8a0005d9de69 = logError(function(arg0, arg1, arg2, arg3) {
            getObject(arg0).uniform2fv(getObject(arg1), getArrayF32FromWasm0(arg2, arg3));
        });
        imports.wbg.__wbg_uniform3fv_3309707bd88ba57a = logError(function(arg0, arg1, arg2, arg3) {
            getObject(arg0).uniform3fv(getObject(arg1), getArrayF32FromWasm0(arg2, arg3));
        });
        imports.wbg.__wbg_uniform4fv_8f8b34c1bf2c810d = logError(function(arg0, arg1, arg2, arg3) {
            getObject(arg0).uniform4fv(getObject(arg1), getArrayF32FromWasm0(arg2, arg3));
        });
        imports.wbg.__wbg_uniformMatrix2fv_a025bc27c170ebaf = logError(function(arg0, arg1, arg2, arg3, arg4) {
            getObject(arg0).uniformMatrix2fv(getObject(arg1), arg2 !== 0, getArrayF32FromWasm0(arg3, arg4));
        });
        imports.wbg.__wbg_uniformMatrix3fv_9002d8bb1622de06 = logError(function(arg0, arg1, arg2, arg3, arg4) {
            getObject(arg0).uniformMatrix3fv(getObject(arg1), arg2 !== 0, getArrayF32FromWasm0(arg3, arg4));
        });
        imports.wbg.__wbg_uniformMatrix4fv_bd73f917605cae31 = logError(function(arg0, arg1, arg2, arg3, arg4) {
            getObject(arg0).uniformMatrix4fv(getObject(arg1), arg2 !== 0, getArrayF32FromWasm0(arg3, arg4));
        });
        imports.wbg.__wbg_activeTexture_ee6eed2472803dd2 = logError(function(arg0, arg1) {
            getObject(arg0).activeTexture(arg1 >>> 0);
        });
        imports.wbg.__wbg_attachShader_c2f7771e6f4b91d8 = logError(function(arg0, arg1, arg2) {
            getObject(arg0).attachShader(getObject(arg1), getObject(arg2));
        });
        imports.wbg.__wbg_bindAttribLocation_49730b954c183e42 = logError(function(arg0, arg1, arg2, arg3, arg4) {
            getObject(arg0).bindAttribLocation(getObject(arg1), arg2 >>> 0, getStringFromWasm0(arg3, arg4));
        });
        imports.wbg.__wbg_bindBuffer_f4ad79795655c1c4 = logError(function(arg0, arg1, arg2) {
            getObject(arg0).bindBuffer(arg1 >>> 0, getObject(arg2));
        });
        imports.wbg.__wbg_bindTexture_751d66bbae4822ab = logError(function(arg0, arg1, arg2) {
            getObject(arg0).bindTexture(arg1 >>> 0, getObject(arg2));
        });
        imports.wbg.__wbg_blendFunc_e6775dade5e99b3e = logError(function(arg0, arg1, arg2) {
            getObject(arg0).blendFunc(arg1 >>> 0, arg2 >>> 0);
        });
        imports.wbg.__wbg_clear_42b42c27d041ce11 = logError(function(arg0, arg1) {
            getObject(arg0).clear(arg1 >>> 0);
        });
        imports.wbg.__wbg_clearColor_ba6ba6886092ab6a = logError(function(arg0, arg1, arg2, arg3, arg4) {
            getObject(arg0).clearColor(arg1, arg2, arg3, arg4);
        });
        imports.wbg.__wbg_compileShader_8aec8947f553f5b6 = logError(function(arg0, arg1) {
            getObject(arg0).compileShader(getObject(arg1));
        });
        imports.wbg.__wbg_createBuffer_f26187e1b465a677 = logError(function(arg0) {
            var ret = getObject(arg0).createBuffer();
            return isLikeNone(ret) ? 0 : addHeapObject(ret);
        });
        imports.wbg.__wbg_createProgram_10f7bf07e21fe904 = logError(function(arg0) {
            var ret = getObject(arg0).createProgram();
            return isLikeNone(ret) ? 0 : addHeapObject(ret);
        });
        imports.wbg.__wbg_createShader_4060106dc88c8bca = logError(function(arg0, arg1) {
            var ret = getObject(arg0).createShader(arg1 >>> 0);
            return isLikeNone(ret) ? 0 : addHeapObject(ret);
        });
        imports.wbg.__wbg_createTexture_d7a4df257a9410a7 = logError(function(arg0) {
            var ret = getObject(arg0).createTexture();
            return isLikeNone(ret) ? 0 : addHeapObject(ret);
        });
        imports.wbg.__wbg_deleteProgram_4df15a60f6fa0bf4 = logError(function(arg0, arg1) {
            getObject(arg0).deleteProgram(getObject(arg1));
        });
        imports.wbg.__wbg_deleteShader_240e5d50373c2602 = logError(function(arg0, arg1) {
            getObject(arg0).deleteShader(getObject(arg1));
        });
        imports.wbg.__wbg_detachShader_b54e5322b7ee3b52 = logError(function(arg0, arg1, arg2) {
            getObject(arg0).detachShader(getObject(arg1), getObject(arg2));
        });
        imports.wbg.__wbg_disable_a1d882f8f4859e70 = logError(function(arg0, arg1) {
            getObject(arg0).disable(arg1 >>> 0);
        });
        imports.wbg.__wbg_enable_24e0ca734ee94d76 = logError(function(arg0, arg1) {
            getObject(arg0).enable(arg1 >>> 0);
        });
        imports.wbg.__wbg_enableVertexAttribArray_2e2bfba7f3a5fb74 = logError(function(arg0, arg1) {
            getObject(arg0).enableVertexAttribArray(arg1 >>> 0);
        });
        imports.wbg.__wbg_generateMipmap_6cf991070cf59d7d = logError(function(arg0, arg1) {
            getObject(arg0).generateMipmap(arg1 >>> 0);
        });
        imports.wbg.__wbg_getActiveAttrib_845a102e41b93acf = logError(function(arg0, arg1, arg2) {
            var ret = getObject(arg0).getActiveAttrib(getObject(arg1), arg2 >>> 0);
            return isLikeNone(ret) ? 0 : addHeapObject(ret);
        });
        imports.wbg.__wbg_getActiveUniform_c5cea6dad88669d4 = logError(function(arg0, arg1, arg2) {
            var ret = getObject(arg0).getActiveUniform(getObject(arg1), arg2 >>> 0);
            return isLikeNone(ret) ? 0 : addHeapObject(ret);
        });
        imports.wbg.__wbg_getAttribLocation_ea61e93124c45a64 = logError(function(arg0, arg1, arg2, arg3) {
            var ret = getObject(arg0).getAttribLocation(getObject(arg1), getStringFromWasm0(arg2, arg3));
            _assertNum(ret);
            return ret;
        });
        imports.wbg.__wbg_getExtension_dc49b5179c4423ee = handleError(function(arg0, arg1, arg2) {
            var ret = getObject(arg0).getExtension(getStringFromWasm0(arg1, arg2));
            return isLikeNone(ret) ? 0 : addHeapObject(ret);
        });
        imports.wbg.__wbg_getParameter_6fe2c5467341febb = handleError(function(arg0, arg1) {
            var ret = getObject(arg0).getParameter(arg1 >>> 0);
            return addHeapObject(ret);
        });
        imports.wbg.__wbg_getProgramInfoLog_ebcdc102c402de8d = logError(function(arg0, arg1, arg2) {
            var ret = getObject(arg1).getProgramInfoLog(getObject(arg2));
            var ptr0 = isLikeNone(ret) ? 0 : passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            var len0 = WASM_VECTOR_LEN;
            getInt32Memory0()[arg0 / 4 + 1] = len0;
            getInt32Memory0()[arg0 / 4 + 0] = ptr0;
        });
        imports.wbg.__wbg_getProgramParameter_02e369d0fe1637e6 = logError(function(arg0, arg1, arg2) {
            var ret = getObject(arg0).getProgramParameter(getObject(arg1), arg2 >>> 0);
            return addHeapObject(ret);
        });
        imports.wbg.__wbg_getShaderInfoLog_932172511c0dfdb7 = logError(function(arg0, arg1, arg2) {
            var ret = getObject(arg1).getShaderInfoLog(getObject(arg2));
            var ptr0 = isLikeNone(ret) ? 0 : passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            var len0 = WASM_VECTOR_LEN;
            getInt32Memory0()[arg0 / 4 + 1] = len0;
            getInt32Memory0()[arg0 / 4 + 0] = ptr0;
        });
        imports.wbg.__wbg_getShaderParameter_4306f019f7eb9f82 = logError(function(arg0, arg1, arg2) {
            var ret = getObject(arg0).getShaderParameter(getObject(arg1), arg2 >>> 0);
            return addHeapObject(ret);
        });
        imports.wbg.__wbg_getUniformLocation_277279212040ec65 = logError(function(arg0, arg1, arg2, arg3) {
            var ret = getObject(arg0).getUniformLocation(getObject(arg1), getStringFromWasm0(arg2, arg3));
            return isLikeNone(ret) ? 0 : addHeapObject(ret);
        });
        imports.wbg.__wbg_linkProgram_9258ef1fcd3afc43 = logError(function(arg0, arg1) {
            getObject(arg0).linkProgram(getObject(arg1));
        });
        imports.wbg.__wbg_pixelStorei_2994b715fe775aca = logError(function(arg0, arg1, arg2) {
            getObject(arg0).pixelStorei(arg1 >>> 0, arg2);
        });
        imports.wbg.__wbg_shaderSource_ef8be775578bf902 = logError(function(arg0, arg1, arg2, arg3) {
            getObject(arg0).shaderSource(getObject(arg1), getStringFromWasm0(arg2, arg3));
        });
        imports.wbg.__wbg_texParameteri_e2db4aa7650962eb = logError(function(arg0, arg1, arg2, arg3) {
            getObject(arg0).texParameteri(arg1 >>> 0, arg2 >>> 0, arg3);
        });
        imports.wbg.__wbg_uniform1f_fb0383a4c61faacf = logError(function(arg0, arg1, arg2) {
            getObject(arg0).uniform1f(getObject(arg1), arg2);
        });
        imports.wbg.__wbg_uniform1i_595e085d9c3aadf2 = logError(function(arg0, arg1, arg2) {
            getObject(arg0).uniform1i(getObject(arg1), arg2);
        });
        imports.wbg.__wbg_uniform2f_ecf476a0ffa61198 = logError(function(arg0, arg1, arg2, arg3) {
            getObject(arg0).uniform2f(getObject(arg1), arg2, arg3);
        });
        imports.wbg.__wbg_uniform3f_eb279ea2d866942a = logError(function(arg0, arg1, arg2, arg3, arg4) {
            getObject(arg0).uniform3f(getObject(arg1), arg2, arg3, arg4);
        });
        imports.wbg.__wbg_uniform4f_d948981d4592be6a = logError(function(arg0, arg1, arg2, arg3, arg4, arg5) {
            getObject(arg0).uniform4f(getObject(arg1), arg2, arg3, arg4, arg5);
        });
        imports.wbg.__wbg_useProgram_67487c5ef197884d = logError(function(arg0, arg1) {
            getObject(arg0).useProgram(getObject(arg1));
        });
        imports.wbg.__wbg_vertexAttribPointer_6e7553ee415848fe = logError(function(arg0, arg1, arg2, arg3, arg4, arg5, arg6) {
            getObject(arg0).vertexAttribPointer(arg1 >>> 0, arg2, arg3 >>> 0, arg4 !== 0, arg5, arg6);
        });
        imports.wbg.__wbg_viewport_5f99aff932f780aa = logError(function(arg0, arg1, arg2, arg3, arg4) {
            getObject(arg0).viewport(arg1, arg2, arg3, arg4);
        });
        imports.wbg.__wbg_instanceof_HtmlElement_8306a9fea71295d9 = logError(function(arg0) {
            var ret = getObject(arg0) instanceof HTMLElement;
            _assertBoolean(ret);
            return ret;
        });
        imports.wbg.__wbg_setonload_f16abc5f9b3b40ec = logError(function(arg0, arg1) {
            getObject(arg0).onload = getObject(arg1);
        });
        imports.wbg.__wbg_setonerror_f653b63b8e04fc79 = logError(function(arg0, arg1) {
            getObject(arg0).onerror = getObject(arg1);
        });
        imports.wbg.__wbg_new_a9ed297b8e18ac77 = handleError(function(arg0, arg1) {
            var ret = new Event(getStringFromWasm0(arg0, arg1));
            return addHeapObject(ret);
        });
        imports.wbg.__wbg_setsrc_07b939013b247d9f = logError(function(arg0, arg1, arg2) {
            getObject(arg0).src = getStringFromWasm0(arg1, arg2);
        });
        imports.wbg.__wbg_setcrossOrigin_5b4a9abe32c8ac80 = logError(function(arg0, arg1, arg2) {
            getObject(arg0).crossOrigin = arg1 === 0 ? undefined : getStringFromWasm0(arg1, arg2);
        });
        imports.wbg.__wbg_width_c7ef8c1c3bdea4c4 = logError(function(arg0) {
            var ret = getObject(arg0).width;
            _assertNum(ret);
            return ret;
        });
        imports.wbg.__wbg_height_3849c376a3b4e5e3 = logError(function(arg0) {
            var ret = getObject(arg0).height;
            _assertNum(ret);
            return ret;
        });
        imports.wbg.__wbg_new_3b07660a1508fd64 = handleError(function() {
            var ret = new Image();
            return addHeapObject(ret);
        });
        imports.wbg.__wbg_width_6aa5a8da1d0e02a2 = logError(function(arg0) {
            var ret = getObject(arg0).width;
            _assertNum(ret);
            return ret;
        });
        imports.wbg.__wbg_height_a77b87a267b35e27 = logError(function(arg0) {
            var ret = getObject(arg0).height;
            _assertNum(ret);
            return ret;
        });
        imports.wbg.__wbg_width_baf98e9ea0f77441 = logError(function(arg0) {
            var ret = getObject(arg0).width;
            _assertNum(ret);
            return ret;
        });
        imports.wbg.__wbg_height_c2dfc6554d26b24a = logError(function(arg0) {
            var ret = getObject(arg0).height;
            _assertNum(ret);
            return ret;
        });
        imports.wbg.__wbg_width_dcd9a38333a0c2cd = logError(function(arg0) {
            var ret = getObject(arg0).width;
            _assertNum(ret);
            return ret;
        });
        imports.wbg.__wbg_height_a69e996dc79b4289 = logError(function(arg0) {
            var ret = getObject(arg0).height;
            _assertNum(ret);
            return ret;
        });
        imports.wbg.__wbg_size_59cf369c603c1a21 = logError(function(arg0) {
            var ret = getObject(arg0).size;
            _assertNum(ret);
            return ret;
        });
        imports.wbg.__wbg_type_33cd5642bb5a40c1 = logError(function(arg0) {
            var ret = getObject(arg0).type;
            _assertNum(ret);
            return ret;
        });
        imports.wbg.__wbg_name_3d54ef5899e7c84f = logError(function(arg0, arg1) {
            var ret = getObject(arg1).name;
            var ptr0 = passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            var len0 = WASM_VECTOR_LEN;
            getInt32Memory0()[arg0 / 4 + 1] = len0;
            getInt32Memory0()[arg0 / 4 + 0] = ptr0;
        });
        imports.wbg.__wbg_newnoargs_8aad4a6554f38345 = logError(function(arg0, arg1) {
            var ret = new Function(getStringFromWasm0(arg0, arg1));
            return addHeapObject(ret);
        });
        imports.wbg.__wbg_call_1f85aaa5836dfb23 = handleError(function(arg0, arg1) {
            var ret = getObject(arg0).call(getObject(arg1));
            return addHeapObject(ret);
        });
        imports.wbg.__wbg_new_d6227c3c833572bb = logError(function() {
            var ret = new Object();
            return addHeapObject(ret);
        });
        imports.wbg.__wbg_resolve_708df7651c8929b8 = logError(function(arg0) {
            var ret = Promise.resolve(getObject(arg0));
            return addHeapObject(ret);
        });
        imports.wbg.__wbg_then_8c23dce80c84c8fb = logError(function(arg0, arg1) {
            var ret = getObject(arg0).then(getObject(arg1));
            return addHeapObject(ret);
        });
        imports.wbg.__wbg_then_300153bb889a5b4b = logError(function(arg0, arg1, arg2) {
            var ret = getObject(arg0).then(getObject(arg1), getObject(arg2));
            return addHeapObject(ret);
        });
        imports.wbg.__wbg_globalThis_c6de1d938e089cf0 = handleError(function() {
            var ret = globalThis.globalThis;
            return addHeapObject(ret);
        });
        imports.wbg.__wbg_self_c0d3a5923e013647 = handleError(function() {
            var ret = self.self;
            return addHeapObject(ret);
        });
        imports.wbg.__wbg_window_7ee6c8be3432927d = handleError(function() {
            var ret = window.window;
            return addHeapObject(ret);
        });
        imports.wbg.__wbg_global_c9a01ce4680907f8 = handleError(function() {
            var ret = global.global;
            return addHeapObject(ret);
        });
        imports.wbg.__wbg_new_470473004db6a289 = logError(function(arg0) {
            var ret = new Float32Array(getObject(arg0));
            return addHeapObject(ret);
        });
        imports.wbg.__wbg_newwithbyteoffsetandlength_a31622ccc380e8b4 = logError(function(arg0, arg1, arg2) {
            var ret = new Float32Array(getObject(arg0), arg1 >>> 0, arg2 >>> 0);
            return addHeapObject(ret);
        });
        imports.wbg.__wbg_length_2f682a6b8ac0fb07 = logError(function(arg0) {
            var ret = getObject(arg0).length;
            _assertNum(ret);
            return ret;
        });
        imports.wbg.__wbg_set_47b2beca3d5c9e3f = logError(function(arg0, arg1, arg2) {
            getObject(arg0).set(getObject(arg1), arg2 >>> 0);
        });
        imports.wbg.__wbg_buffer_eb5185aa4a8e9c62 = logError(function(arg0) {
            var ret = getObject(arg0).buffer;
            return addHeapObject(ret);
        });
        imports.wbg.__wbindgen_object_drop_ref = function(arg0) {
            takeObject(arg0);
        };
        imports.wbg.__wbg_set_6a666216929b0387 = handleError(function(arg0, arg1, arg2) {
            var ret = Reflect.set(getObject(arg0), getObject(arg1), getObject(arg2));
            _assertBoolean(ret);
            return ret;
        });
        imports.wbg.__wbg_self_1b7a39e3a92c949c = handleError(function() {
            var ret = self.self;
            return addHeapObject(ret);
        });
        imports.wbg.__wbg_crypto_968f1772287e2df0 = logError(function(arg0) {
            var ret = getObject(arg0).crypto;
            return addHeapObject(ret);
        });
        imports.wbg.__wbg_getRandomValues_a3d34b4fee3c2869 = logError(function(arg0) {
            var ret = getObject(arg0).getRandomValues;
            return addHeapObject(ret);
        });
        imports.wbg.__wbg_getRandomValues_f5e14ab7ac8e995d = logError(function(arg0, arg1, arg2) {
            getObject(arg0).getRandomValues(getArrayU8FromWasm0(arg1, arg2));
        });
        imports.wbg.__wbg_require_604837428532a733 = logError(function(arg0, arg1) {
            var ret = require(getStringFromWasm0(arg0, arg1));
            return addHeapObject(ret);
        });
        imports.wbg.__wbg_randomFillSync_d5bd2d655fdf256a = logError(function(arg0, arg1, arg2) {
            getObject(arg0).randomFillSync(getArrayU8FromWasm0(arg1, arg2));
        });
        imports.wbg.__wbindgen_number_get = function(arg0, arg1) {
            const obj = getObject(arg1);
            var ret = typeof(obj) === 'number' ? obj : undefined;
            if (!isLikeNone(ret)) {
                _assertNum(ret);
            }
            getFloat64Memory0()[arg0 / 8 + 1] = isLikeNone(ret) ? 0 : ret;
            getInt32Memory0()[arg0 / 4 + 0] = !isLikeNone(ret);
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
            _assertNum(ret);
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
        imports.wbg.__wbindgen_closure_wrapper1057 = logError(function(arg0, arg1, arg2) {
            var ret = makeMutClosure(arg0, arg1, 157, __wbg_adapter_32);
            return addHeapObject(ret);
        });
        imports.wbg.__wbindgen_closure_wrapper7530 = logError(function(arg0, arg1, arg2) {
            var ret = makeMutClosure(arg0, arg1, 241, __wbg_adapter_29);
            return addHeapObject(ret);
        });
        imports.wbg.__wbindgen_closure_wrapper1055 = logError(function(arg0, arg1, arg2) {
            var ret = makeMutClosure(arg0, arg1, 159, __wbg_adapter_35);
            return addHeapObject(ret);
        });
        imports.wbg.__wbindgen_closure_wrapper5711 = logError(function(arg0, arg1, arg2) {
            var ret = makeMutClosure(arg0, arg1, 214, __wbg_adapter_26);
            return addHeapObject(ret);
        });

        if (typeof input === 'string' || (typeof Request === 'function' && input instanceof Request) || (typeof URL === 'function' && input instanceof URL)) {
            input = fetch(input);
        }

        const { instance, module } = await load(await input, imports);

        wasm = instance.exports;
        init.__wbindgen_wasm_module = module;
        wasm.__wbindgen_start();
        return wasm;
    }

    init("/wasm/shipyard_demo.wasm").catch(console.error);

}());
//# sourceMappingURL=index.js.map
