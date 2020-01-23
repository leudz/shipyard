
let wasm;

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

const heap = new Array(32);

heap.fill(undefined);

heap.push(undefined, null, true, false);

let heap_next = heap.length;

function addHeapObject(obj) {
    if (heap_next === heap.length) heap.push(heap.length + 1);
    const idx = heap_next;
    heap_next = heap[idx];

    if (typeof(heap_next) !== 'number') throw new Error('corrupt heap');

    heap[idx] = obj;
    return idx;
}

function getObject(idx) { return heap[idx]; }

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

function logError(e) {
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
function __wbg_adapter_28(arg0, arg1) {
    _assertNum(arg0);
    _assertNum(arg1);
    wasm._dyn_core__ops__function__FnMut_____Output___R_as_wasm_bindgen__closure__WasmClosure___describe__invoke__hb4884a25ad42d845(arg0, arg1);
}

let stack_pointer = 32;

function addBorrowedObject(obj) {
    if (stack_pointer == 1) throw new Error('out of js stack');
    heap[--stack_pointer] = obj;
    return stack_pointer;
}
function __wbg_adapter_31(arg0, arg1, arg2) {
    try {
        _assertNum(arg0);
        _assertNum(arg1);
        wasm._dyn_core__ops__function__FnMut___A____Output___R_as_wasm_bindgen__closure__WasmClosure___describe__invoke__he0d96c95df03b0de(arg0, arg1, addBorrowedObject(arg2));
    } finally {
        heap[stack_pointer++] = undefined;
    }
}

function __wbg_adapter_34(arg0, arg1, arg2) {
    _assertNum(arg0);
    _assertNum(arg1);
    wasm._dyn_core__ops__function__FnMut__A____Output___R_as_wasm_bindgen__closure__WasmClosure___describe__invoke__h10026a2db627f39d(arg0, arg1, arg2);
}

function __wbg_adapter_37(arg0, arg1, arg2) {
    _assertNum(arg0);
    _assertNum(arg1);
    wasm._dyn_core__ops__function__FnMut__A____Output___R_as_wasm_bindgen__closure__WasmClosure___describe__invoke__h11e069d64f03e03b(arg0, arg1, addHeapObject(arg2));
}

/**
* @returns {any}
*/
export function run() {
    var ret = wasm.run();
    return takeObject(ret);
}

function handleError(e) {
    wasm.__wbindgen_exn_store(addHeapObject(e));
}

function getArrayU8FromWasm0(ptr, len) {
    return getUint8Memory0().subarray(ptr / 1, ptr / 1 + len);
}
function __wbg_adapter_263(arg0, arg1, arg2, arg3) {
    _assertNum(arg0);
    _assertNum(arg1);
    wasm.wasm_bindgen__convert__closures__invoke2_mut__h01471411c11fcb8e(arg0, arg1, addHeapObject(arg2), addHeapObject(arg3));
}

function init(module) {
    if (typeof module === 'undefined') {
        module = import.meta.url.replace(/\.js$/, '_bg.wasm');
    }
    let result;
    const imports = {};
    imports.wbg = {};
    imports.wbg.__wbindgen_string_new = function(arg0, arg1) {
        var ret = getStringFromWasm0(arg0, arg1);
        return addHeapObject(ret);
    };
    imports.wbg.__wbindgen_object_clone_ref = function(arg0) {
        var ret = getObject(arg0);
        return addHeapObject(ret);
    };
    imports.wbg.__wbindgen_object_drop_ref = function(arg0) {
        takeObject(arg0);
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
    imports.wbg.__wbg_error_4bb6c2a97407129a = function(arg0, arg1) {
        try {
            try {
                console.error(getStringFromWasm0(arg0, arg1));
            } finally {
                wasm.__wbindgen_free(arg0, arg1);
            }
        } catch (e) {
            logError(e)
        }
    };
    imports.wbg.__wbg_new_59cb74e423758ede = function() {
        try {
            var ret = new Error();
            return addHeapObject(ret);
        } catch (e) {
            logError(e)
        }
    };
    imports.wbg.__wbg_stack_558ba5917b466edd = function(arg0, arg1) {
        try {
            var ret = getObject(arg1).stack;
            var ptr0 = passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            var len0 = WASM_VECTOR_LEN;
            getInt32Memory0()[arg0 / 4 + 1] = len0;
            getInt32Memory0()[arg0 / 4 + 0] = ptr0;
        } catch (e) {
            logError(e)
        }
    };
    imports.wbg.__wbindgen_is_undefined = function(arg0) {
        var ret = getObject(arg0) === undefined;
        _assertBoolean(ret);
        return ret;
    };
    imports.wbg.__wbg_new_68adb0d58759a4ed = function() {
        try {
            var ret = new Object();
            return addHeapObject(ret);
        } catch (e) {
            logError(e)
        }
    };
    imports.wbg.__wbg_set_2e79e744454afade = function(arg0, arg1, arg2) {
        try {
            getObject(arg0)[takeObject(arg1)] = takeObject(arg2);
        } catch (e) {
            logError(e)
        }
    };
    imports.wbg.__wbg_self_1b7a39e3a92c949c = function() {
        try {
            try {
                var ret = self.self;
                return addHeapObject(ret);
            } catch (e) {
                handleError(e)
            }
        } catch (e) {
            logError(e)
        }
    };
    imports.wbg.__wbg_crypto_968f1772287e2df0 = function(arg0) {
        try {
            var ret = getObject(arg0).crypto;
            return addHeapObject(ret);
        } catch (e) {
            logError(e)
        }
    };
    imports.wbg.__wbg_getRandomValues_a3d34b4fee3c2869 = function(arg0) {
        try {
            var ret = getObject(arg0).getRandomValues;
            return addHeapObject(ret);
        } catch (e) {
            logError(e)
        }
    };
    imports.wbg.__wbg_getRandomValues_f5e14ab7ac8e995d = function(arg0, arg1, arg2) {
        try {
            getObject(arg0).getRandomValues(getArrayU8FromWasm0(arg1, arg2));
        } catch (e) {
            logError(e)
        }
    };
    imports.wbg.__wbg_require_604837428532a733 = function(arg0, arg1) {
        try {
            var ret = require(getStringFromWasm0(arg0, arg1));
            return addHeapObject(ret);
        } catch (e) {
            logError(e)
        }
    };
    imports.wbg.__wbg_randomFillSync_d5bd2d655fdf256a = function(arg0, arg1, arg2) {
        try {
            getObject(arg0).randomFillSync(getArrayU8FromWasm0(arg1, arg2));
        } catch (e) {
            logError(e)
        }
    };
    imports.wbg.__widl_f_create_element_Document = function(arg0, arg1, arg2) {
        try {
            try {
                var ret = getObject(arg0).createElement(getStringFromWasm0(arg1, arg2));
                return addHeapObject(ret);
            } catch (e) {
                handleError(e)
            }
        } catch (e) {
            logError(e)
        }
    };
    imports.wbg.__widl_f_body_Document = function(arg0) {
        try {
            var ret = getObject(arg0).body;
            return isLikeNone(ret) ? 0 : addHeapObject(ret);
        } catch (e) {
            logError(e)
        }
    };
    imports.wbg.__widl_f_set_class_name_Element = function(arg0, arg1, arg2) {
        try {
            getObject(arg0).className = getStringFromWasm0(arg1, arg2);
        } catch (e) {
            logError(e)
        }
    };
    imports.wbg.__widl_f_new_Event = function(arg0, arg1) {
        try {
            try {
                var ret = new Event(getStringFromWasm0(arg0, arg1));
                return addHeapObject(ret);
            } catch (e) {
                handleError(e)
            }
        } catch (e) {
            logError(e)
        }
    };
    imports.wbg.__widl_f_add_event_listener_with_callback_and_add_event_listener_options_EventTarget = function(arg0, arg1, arg2, arg3, arg4) {
        try {
            try {
                getObject(arg0).addEventListener(getStringFromWasm0(arg1, arg2), getObject(arg3), getObject(arg4));
            } catch (e) {
                handleError(e)
            }
        } catch (e) {
            logError(e)
        }
    };
    imports.wbg.__widl_f_remove_event_listener_with_callback_and_bool_EventTarget = function(arg0, arg1, arg2, arg3, arg4) {
        try {
            try {
                getObject(arg0).removeEventListener(getStringFromWasm0(arg1, arg2), getObject(arg3), arg4 !== 0);
            } catch (e) {
                handleError(e)
            }
        } catch (e) {
            logError(e)
        }
    };
    imports.wbg.__widl_instanceof_HTMLCanvasElement = function(arg0) {
        try {
            var ret = getObject(arg0) instanceof HTMLCanvasElement;
            _assertBoolean(ret);
            return ret;
        } catch (e) {
            logError(e)
        }
    };
    imports.wbg.__widl_f_get_context_HTMLCanvasElement = function(arg0, arg1, arg2) {
        try {
            try {
                var ret = getObject(arg0).getContext(getStringFromWasm0(arg1, arg2));
                return isLikeNone(ret) ? 0 : addHeapObject(ret);
            } catch (e) {
                handleError(e)
            }
        } catch (e) {
            logError(e)
        }
    };
    imports.wbg.__widl_f_get_context_with_context_options_HTMLCanvasElement = function(arg0, arg1, arg2, arg3) {
        try {
            try {
                var ret = getObject(arg0).getContext(getStringFromWasm0(arg1, arg2), getObject(arg3));
                return isLikeNone(ret) ? 0 : addHeapObject(ret);
            } catch (e) {
                handleError(e)
            }
        } catch (e) {
            logError(e)
        }
    };
    imports.wbg.__widl_f_width_HTMLCanvasElement = function(arg0) {
        try {
            var ret = getObject(arg0).width;
            _assertNum(ret);
            return ret;
        } catch (e) {
            logError(e)
        }
    };
    imports.wbg.__widl_f_set_width_HTMLCanvasElement = function(arg0, arg1) {
        try {
            getObject(arg0).width = arg1 >>> 0;
        } catch (e) {
            logError(e)
        }
    };
    imports.wbg.__widl_f_height_HTMLCanvasElement = function(arg0) {
        try {
            var ret = getObject(arg0).height;
            _assertNum(ret);
            return ret;
        } catch (e) {
            logError(e)
        }
    };
    imports.wbg.__widl_f_set_height_HTMLCanvasElement = function(arg0, arg1) {
        try {
            getObject(arg0).height = arg1 >>> 0;
        } catch (e) {
            logError(e)
        }
    };
    imports.wbg.__widl_instanceof_HTMLElement = function(arg0) {
        try {
            var ret = getObject(arg0) instanceof HTMLElement;
            _assertBoolean(ret);
            return ret;
        } catch (e) {
            logError(e)
        }
    };
    imports.wbg.__widl_f_set_onload_HTMLElement = function(arg0, arg1) {
        try {
            getObject(arg0).onload = getObject(arg1);
        } catch (e) {
            logError(e)
        }
    };
    imports.wbg.__widl_f_set_onerror_HTMLElement = function(arg0, arg1) {
        try {
            getObject(arg0).onerror = getObject(arg1);
        } catch (e) {
            logError(e)
        }
    };
    imports.wbg.__widl_f_new_Image = function() {
        try {
            try {
                var ret = new Image();
                return addHeapObject(ret);
            } catch (e) {
                handleError(e)
            }
        } catch (e) {
            logError(e)
        }
    };
    imports.wbg.__widl_f_set_src_HTMLImageElement = function(arg0, arg1, arg2) {
        try {
            getObject(arg0).src = getStringFromWasm0(arg1, arg2);
        } catch (e) {
            logError(e)
        }
    };
    imports.wbg.__widl_f_set_cross_origin_HTMLImageElement = function(arg0, arg1, arg2) {
        try {
            getObject(arg0).crossOrigin = arg1 === 0 ? undefined : getStringFromWasm0(arg1, arg2);
        } catch (e) {
            logError(e)
        }
    };
    imports.wbg.__widl_f_width_HTMLImageElement = function(arg0) {
        try {
            var ret = getObject(arg0).width;
            _assertNum(ret);
            return ret;
        } catch (e) {
            logError(e)
        }
    };
    imports.wbg.__widl_f_height_HTMLImageElement = function(arg0) {
        try {
            var ret = getObject(arg0).height;
            _assertNum(ret);
            return ret;
        } catch (e) {
            logError(e)
        }
    };
    imports.wbg.__widl_f_width_HTMLVideoElement = function(arg0) {
        try {
            var ret = getObject(arg0).width;
            _assertNum(ret);
            return ret;
        } catch (e) {
            logError(e)
        }
    };
    imports.wbg.__widl_f_height_HTMLVideoElement = function(arg0) {
        try {
            var ret = getObject(arg0).height;
            _assertNum(ret);
            return ret;
        } catch (e) {
            logError(e)
        }
    };
    imports.wbg.__widl_f_width_ImageBitmap = function(arg0) {
        try {
            var ret = getObject(arg0).width;
            _assertNum(ret);
            return ret;
        } catch (e) {
            logError(e)
        }
    };
    imports.wbg.__widl_f_height_ImageBitmap = function(arg0) {
        try {
            var ret = getObject(arg0).height;
            _assertNum(ret);
            return ret;
        } catch (e) {
            logError(e)
        }
    };
    imports.wbg.__widl_f_width_ImageData = function(arg0) {
        try {
            var ret = getObject(arg0).width;
            _assertNum(ret);
            return ret;
        } catch (e) {
            logError(e)
        }
    };
    imports.wbg.__widl_f_height_ImageData = function(arg0) {
        try {
            var ret = getObject(arg0).height;
            _assertNum(ret);
            return ret;
        } catch (e) {
            logError(e)
        }
    };
    imports.wbg.__widl_f_origin_Location = function(arg0, arg1) {
        try {
            try {
                var ret = getObject(arg1).origin;
                var ptr0 = passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
                var len0 = WASM_VECTOR_LEN;
                getInt32Memory0()[arg0 / 4 + 1] = len0;
                getInt32Memory0()[arg0 / 4 + 0] = ptr0;
            } catch (e) {
                handleError(e)
            }
        } catch (e) {
            logError(e)
        }
    };
    imports.wbg.__widl_f_append_child_Node = function(arg0, arg1) {
        try {
            try {
                var ret = getObject(arg0).appendChild(getObject(arg1));
                return addHeapObject(ret);
            } catch (e) {
                handleError(e)
            }
        } catch (e) {
            logError(e)
        }
    };
    imports.wbg.__widl_f_remove_child_Node = function(arg0, arg1) {
        try {
            try {
                var ret = getObject(arg0).removeChild(getObject(arg1));
                return addHeapObject(ret);
            } catch (e) {
                handleError(e)
            }
        } catch (e) {
            logError(e)
        }
    };
    imports.wbg.__widl_f_set_text_content_Node = function(arg0, arg1, arg2) {
        try {
            getObject(arg0).textContent = arg1 === 0 ? undefined : getStringFromWasm0(arg1, arg2);
        } catch (e) {
            logError(e)
        }
    };
    imports.wbg.__widl_f_now_Performance = function(arg0) {
        try {
            var ret = getObject(arg0).now();
            return ret;
        } catch (e) {
            logError(e)
        }
    };
    imports.wbg.__widl_f_new_with_str_Request = function(arg0, arg1) {
        try {
            try {
                var ret = new Request(getStringFromWasm0(arg0, arg1));
                return addHeapObject(ret);
            } catch (e) {
                handleError(e)
            }
        } catch (e) {
            logError(e)
        }
    };
    imports.wbg.__widl_instanceof_Response = function(arg0) {
        try {
            var ret = getObject(arg0) instanceof Response;
            _assertBoolean(ret);
            return ret;
        } catch (e) {
            logError(e)
        }
    };
    imports.wbg.__widl_f_text_Response = function(arg0) {
        try {
            try {
                var ret = getObject(arg0).text();
                return addHeapObject(ret);
            } catch (e) {
                handleError(e)
            }
        } catch (e) {
            logError(e)
        }
    };
    imports.wbg.__widl_f_new_URL = function(arg0, arg1) {
        try {
            try {
                var ret = new URL(getStringFromWasm0(arg0, arg1));
                return addHeapObject(ret);
            } catch (e) {
                handleError(e)
            }
        } catch (e) {
            logError(e)
        }
    };
    imports.wbg.__widl_f_origin_URL = function(arg0, arg1) {
        try {
            var ret = getObject(arg1).origin;
            var ptr0 = passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            var len0 = WASM_VECTOR_LEN;
            getInt32Memory0()[arg0 / 4 + 1] = len0;
            getInt32Memory0()[arg0 / 4 + 0] = ptr0;
        } catch (e) {
            logError(e)
        }
    };
    imports.wbg.__widl_f_type_WebGLActiveInfo = function(arg0) {
        try {
            var ret = getObject(arg0).type;
            _assertNum(ret);
            return ret;
        } catch (e) {
            logError(e)
        }
    };
    imports.wbg.__widl_f_name_WebGLActiveInfo = function(arg0, arg1) {
        try {
            var ret = getObject(arg1).name;
            var ptr0 = passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            var len0 = WASM_VECTOR_LEN;
            getInt32Memory0()[arg0 / 4 + 1] = len0;
            getInt32Memory0()[arg0 / 4 + 0] = ptr0;
        } catch (e) {
            logError(e)
        }
    };
    imports.wbg.__widl_instanceof_WebGLRenderingContext = function(arg0) {
        try {
            var ret = getObject(arg0) instanceof WebGLRenderingContext;
            _assertBoolean(ret);
            return ret;
        } catch (e) {
            logError(e)
        }
    };
    imports.wbg.__widl_f_buffer_data_with_array_buffer_view_WebGLRenderingContext = function(arg0, arg1, arg2, arg3) {
        try {
            getObject(arg0).bufferData(arg1 >>> 0, getObject(arg2), arg3 >>> 0);
        } catch (e) {
            logError(e)
        }
    };
    imports.wbg.__widl_f_tex_image_2d_with_i32_and_i32_and_i32_and_format_and_type_and_opt_array_buffer_view_WebGLRenderingContext = function(arg0, arg1, arg2, arg3, arg4, arg5, arg6, arg7, arg8, arg9) {
        try {
            try {
                getObject(arg0).texImage2D(arg1 >>> 0, arg2, arg3, arg4, arg5, arg6, arg7 >>> 0, arg8 >>> 0, getObject(arg9));
            } catch (e) {
                handleError(e)
            }
        } catch (e) {
            logError(e)
        }
    };
    imports.wbg.__widl_f_tex_image_2d_with_u32_and_u32_and_image_bitmap_WebGLRenderingContext = function(arg0, arg1, arg2, arg3, arg4, arg5, arg6) {
        try {
            try {
                getObject(arg0).texImage2D(arg1 >>> 0, arg2, arg3, arg4 >>> 0, arg5 >>> 0, getObject(arg6));
            } catch (e) {
                handleError(e)
            }
        } catch (e) {
            logError(e)
        }
    };
    imports.wbg.__widl_f_tex_image_2d_with_u32_and_u32_and_image_data_WebGLRenderingContext = function(arg0, arg1, arg2, arg3, arg4, arg5, arg6) {
        try {
            try {
                getObject(arg0).texImage2D(arg1 >>> 0, arg2, arg3, arg4 >>> 0, arg5 >>> 0, getObject(arg6));
            } catch (e) {
                handleError(e)
            }
        } catch (e) {
            logError(e)
        }
    };
    imports.wbg.__widl_f_tex_image_2d_with_u32_and_u32_and_image_WebGLRenderingContext = function(arg0, arg1, arg2, arg3, arg4, arg5, arg6) {
        try {
            try {
                getObject(arg0).texImage2D(arg1 >>> 0, arg2, arg3, arg4 >>> 0, arg5 >>> 0, getObject(arg6));
            } catch (e) {
                handleError(e)
            }
        } catch (e) {
            logError(e)
        }
    };
    imports.wbg.__widl_f_tex_image_2d_with_u32_and_u32_and_canvas_WebGLRenderingContext = function(arg0, arg1, arg2, arg3, arg4, arg5, arg6) {
        try {
            try {
                getObject(arg0).texImage2D(arg1 >>> 0, arg2, arg3, arg4 >>> 0, arg5 >>> 0, getObject(arg6));
            } catch (e) {
                handleError(e)
            }
        } catch (e) {
            logError(e)
        }
    };
    imports.wbg.__widl_f_tex_image_2d_with_u32_and_u32_and_video_WebGLRenderingContext = function(arg0, arg1, arg2, arg3, arg4, arg5, arg6) {
        try {
            try {
                getObject(arg0).texImage2D(arg1 >>> 0, arg2, arg3, arg4 >>> 0, arg5 >>> 0, getObject(arg6));
            } catch (e) {
                handleError(e)
            }
        } catch (e) {
            logError(e)
        }
    };
    imports.wbg.__widl_f_attach_shader_WebGLRenderingContext = function(arg0, arg1, arg2) {
        try {
            getObject(arg0).attachShader(getObject(arg1), getObject(arg2));
        } catch (e) {
            logError(e)
        }
    };
    imports.wbg.__widl_f_bind_buffer_WebGLRenderingContext = function(arg0, arg1, arg2) {
        try {
            getObject(arg0).bindBuffer(arg1 >>> 0, getObject(arg2));
        } catch (e) {
            logError(e)
        }
    };
    imports.wbg.__widl_f_bind_texture_WebGLRenderingContext = function(arg0, arg1, arg2) {
        try {
            getObject(arg0).bindTexture(arg1 >>> 0, getObject(arg2));
        } catch (e) {
            logError(e)
        }
    };
    imports.wbg.__widl_f_clear_WebGLRenderingContext = function(arg0, arg1) {
        try {
            getObject(arg0).clear(arg1 >>> 0);
        } catch (e) {
            logError(e)
        }
    };
    imports.wbg.__widl_f_clear_color_WebGLRenderingContext = function(arg0, arg1, arg2, arg3, arg4) {
        try {
            getObject(arg0).clearColor(arg1, arg2, arg3, arg4);
        } catch (e) {
            logError(e)
        }
    };
    imports.wbg.__widl_f_compile_shader_WebGLRenderingContext = function(arg0, arg1) {
        try {
            getObject(arg0).compileShader(getObject(arg1));
        } catch (e) {
            logError(e)
        }
    };
    imports.wbg.__widl_f_create_buffer_WebGLRenderingContext = function(arg0) {
        try {
            var ret = getObject(arg0).createBuffer();
            return isLikeNone(ret) ? 0 : addHeapObject(ret);
        } catch (e) {
            logError(e)
        }
    };
    imports.wbg.__widl_f_create_program_WebGLRenderingContext = function(arg0) {
        try {
            var ret = getObject(arg0).createProgram();
            return isLikeNone(ret) ? 0 : addHeapObject(ret);
        } catch (e) {
            logError(e)
        }
    };
    imports.wbg.__widl_f_create_shader_WebGLRenderingContext = function(arg0, arg1) {
        try {
            var ret = getObject(arg0).createShader(arg1 >>> 0);
            return isLikeNone(ret) ? 0 : addHeapObject(ret);
        } catch (e) {
            logError(e)
        }
    };
    imports.wbg.__widl_f_create_texture_WebGLRenderingContext = function(arg0) {
        try {
            var ret = getObject(arg0).createTexture();
            return isLikeNone(ret) ? 0 : addHeapObject(ret);
        } catch (e) {
            logError(e)
        }
    };
    imports.wbg.__widl_f_delete_program_WebGLRenderingContext = function(arg0, arg1) {
        try {
            getObject(arg0).deleteProgram(getObject(arg1));
        } catch (e) {
            logError(e)
        }
    };
    imports.wbg.__widl_f_delete_shader_WebGLRenderingContext = function(arg0, arg1) {
        try {
            getObject(arg0).deleteShader(getObject(arg1));
        } catch (e) {
            logError(e)
        }
    };
    imports.wbg.__widl_f_detach_shader_WebGLRenderingContext = function(arg0, arg1, arg2) {
        try {
            getObject(arg0).detachShader(getObject(arg1), getObject(arg2));
        } catch (e) {
            logError(e)
        }
    };
    imports.wbg.__widl_f_enable_vertex_attrib_array_WebGLRenderingContext = function(arg0, arg1) {
        try {
            getObject(arg0).enableVertexAttribArray(arg1 >>> 0);
        } catch (e) {
            logError(e)
        }
    };
    imports.wbg.__widl_f_generate_mipmap_WebGLRenderingContext = function(arg0, arg1) {
        try {
            getObject(arg0).generateMipmap(arg1 >>> 0);
        } catch (e) {
            logError(e)
        }
    };
    imports.wbg.__widl_f_get_active_attrib_WebGLRenderingContext = function(arg0, arg1, arg2) {
        try {
            var ret = getObject(arg0).getActiveAttrib(getObject(arg1), arg2 >>> 0);
            return isLikeNone(ret) ? 0 : addHeapObject(ret);
        } catch (e) {
            logError(e)
        }
    };
    imports.wbg.__widl_f_get_active_uniform_WebGLRenderingContext = function(arg0, arg1, arg2) {
        try {
            var ret = getObject(arg0).getActiveUniform(getObject(arg1), arg2 >>> 0);
            return isLikeNone(ret) ? 0 : addHeapObject(ret);
        } catch (e) {
            logError(e)
        }
    };
    imports.wbg.__widl_f_get_attrib_location_WebGLRenderingContext = function(arg0, arg1, arg2, arg3) {
        try {
            var ret = getObject(arg0).getAttribLocation(getObject(arg1), getStringFromWasm0(arg2, arg3));
            _assertNum(ret);
            return ret;
        } catch (e) {
            logError(e)
        }
    };
    imports.wbg.__widl_f_get_extension_WebGLRenderingContext = function(arg0, arg1, arg2) {
        try {
            try {
                var ret = getObject(arg0).getExtension(getStringFromWasm0(arg1, arg2));
                return isLikeNone(ret) ? 0 : addHeapObject(ret);
            } catch (e) {
                handleError(e)
            }
        } catch (e) {
            logError(e)
        }
    };
    imports.wbg.__widl_f_get_parameter_WebGLRenderingContext = function(arg0, arg1) {
        try {
            try {
                var ret = getObject(arg0).getParameter(arg1 >>> 0);
                return addHeapObject(ret);
            } catch (e) {
                handleError(e)
            }
        } catch (e) {
            logError(e)
        }
    };
    imports.wbg.__widl_f_get_program_info_log_WebGLRenderingContext = function(arg0, arg1, arg2) {
        try {
            var ret = getObject(arg1).getProgramInfoLog(getObject(arg2));
            var ptr0 = isLikeNone(ret) ? 0 : passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            var len0 = WASM_VECTOR_LEN;
            getInt32Memory0()[arg0 / 4 + 1] = len0;
            getInt32Memory0()[arg0 / 4 + 0] = ptr0;
        } catch (e) {
            logError(e)
        }
    };
    imports.wbg.__widl_f_get_program_parameter_WebGLRenderingContext = function(arg0, arg1, arg2) {
        try {
            var ret = getObject(arg0).getProgramParameter(getObject(arg1), arg2 >>> 0);
            return addHeapObject(ret);
        } catch (e) {
            logError(e)
        }
    };
    imports.wbg.__widl_f_get_shader_info_log_WebGLRenderingContext = function(arg0, arg1, arg2) {
        try {
            var ret = getObject(arg1).getShaderInfoLog(getObject(arg2));
            var ptr0 = isLikeNone(ret) ? 0 : passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            var len0 = WASM_VECTOR_LEN;
            getInt32Memory0()[arg0 / 4 + 1] = len0;
            getInt32Memory0()[arg0 / 4 + 0] = ptr0;
        } catch (e) {
            logError(e)
        }
    };
    imports.wbg.__widl_f_get_shader_parameter_WebGLRenderingContext = function(arg0, arg1, arg2) {
        try {
            var ret = getObject(arg0).getShaderParameter(getObject(arg1), arg2 >>> 0);
            return addHeapObject(ret);
        } catch (e) {
            logError(e)
        }
    };
    imports.wbg.__widl_f_get_uniform_location_WebGLRenderingContext = function(arg0, arg1, arg2, arg3) {
        try {
            var ret = getObject(arg0).getUniformLocation(getObject(arg1), getStringFromWasm0(arg2, arg3));
            return isLikeNone(ret) ? 0 : addHeapObject(ret);
        } catch (e) {
            logError(e)
        }
    };
    imports.wbg.__widl_f_link_program_WebGLRenderingContext = function(arg0, arg1) {
        try {
            getObject(arg0).linkProgram(getObject(arg1));
        } catch (e) {
            logError(e)
        }
    };
    imports.wbg.__widl_f_pixel_storei_WebGLRenderingContext = function(arg0, arg1, arg2) {
        try {
            getObject(arg0).pixelStorei(arg1 >>> 0, arg2);
        } catch (e) {
            logError(e)
        }
    };
    imports.wbg.__widl_f_shader_source_WebGLRenderingContext = function(arg0, arg1, arg2, arg3) {
        try {
            getObject(arg0).shaderSource(getObject(arg1), getStringFromWasm0(arg2, arg3));
        } catch (e) {
            logError(e)
        }
    };
    imports.wbg.__widl_f_tex_parameteri_WebGLRenderingContext = function(arg0, arg1, arg2, arg3) {
        try {
            getObject(arg0).texParameteri(arg1 >>> 0, arg2 >>> 0, arg3);
        } catch (e) {
            logError(e)
        }
    };
    imports.wbg.__widl_f_uniform1i_WebGLRenderingContext = function(arg0, arg1, arg2) {
        try {
            getObject(arg0).uniform1i(getObject(arg1), arg2);
        } catch (e) {
            logError(e)
        }
    };
    imports.wbg.__widl_f_use_program_WebGLRenderingContext = function(arg0, arg1) {
        try {
            getObject(arg0).useProgram(getObject(arg1));
        } catch (e) {
            logError(e)
        }
    };
    imports.wbg.__widl_f_vertex_attrib_pointer_with_f64_WebGLRenderingContext = function(arg0, arg1, arg2, arg3, arg4, arg5, arg6) {
        try {
            getObject(arg0).vertexAttribPointer(arg1 >>> 0, arg2, arg3 >>> 0, arg4 !== 0, arg5, arg6);
        } catch (e) {
            logError(e)
        }
    };
    imports.wbg.__widl_f_viewport_WebGLRenderingContext = function(arg0, arg1, arg2, arg3, arg4) {
        try {
            getObject(arg0).viewport(arg1, arg2, arg3, arg4);
        } catch (e) {
            logError(e)
        }
    };
    imports.wbg.__widl_f_canvas_WebGLRenderingContext = function(arg0) {
        try {
            var ret = getObject(arg0).canvas;
            return isLikeNone(ret) ? 0 : addHeapObject(ret);
        } catch (e) {
            logError(e)
        }
    };
    imports.wbg.__widl_f_drawing_buffer_width_WebGLRenderingContext = function(arg0) {
        try {
            var ret = getObject(arg0).drawingBufferWidth;
            _assertNum(ret);
            return ret;
        } catch (e) {
            logError(e)
        }
    };
    imports.wbg.__widl_f_drawing_buffer_height_WebGLRenderingContext = function(arg0) {
        try {
            var ret = getObject(arg0).drawingBufferHeight;
            _assertNum(ret);
            return ret;
        } catch (e) {
            logError(e)
        }
    };
    imports.wbg.__widl_instanceof_Window = function(arg0) {
        try {
            var ret = getObject(arg0) instanceof Window;
            _assertBoolean(ret);
            return ret;
        } catch (e) {
            logError(e)
        }
    };
    imports.wbg.__widl_f_request_animation_frame_Window = function(arg0, arg1) {
        try {
            try {
                var ret = getObject(arg0).requestAnimationFrame(getObject(arg1));
                _assertNum(ret);
                return ret;
            } catch (e) {
                handleError(e)
            }
        } catch (e) {
            logError(e)
        }
    };
    imports.wbg.__widl_f_document_Window = function(arg0) {
        try {
            var ret = getObject(arg0).document;
            return isLikeNone(ret) ? 0 : addHeapObject(ret);
        } catch (e) {
            logError(e)
        }
    };
    imports.wbg.__widl_f_location_Window = function(arg0) {
        try {
            var ret = getObject(arg0).location;
            return addHeapObject(ret);
        } catch (e) {
            logError(e)
        }
    };
    imports.wbg.__widl_f_inner_width_Window = function(arg0) {
        try {
            try {
                var ret = getObject(arg0).innerWidth;
                return addHeapObject(ret);
            } catch (e) {
                handleError(e)
            }
        } catch (e) {
            logError(e)
        }
    };
    imports.wbg.__widl_f_inner_height_Window = function(arg0) {
        try {
            try {
                var ret = getObject(arg0).innerHeight;
                return addHeapObject(ret);
            } catch (e) {
                handleError(e)
            }
        } catch (e) {
            logError(e)
        }
    };
    imports.wbg.__widl_f_performance_Window = function(arg0) {
        try {
            var ret = getObject(arg0).performance;
            return isLikeNone(ret) ? 0 : addHeapObject(ret);
        } catch (e) {
            logError(e)
        }
    };
    imports.wbg.__widl_f_fetch_with_request_Window = function(arg0, arg1) {
        try {
            var ret = getObject(arg0).fetch(getObject(arg1));
            return addHeapObject(ret);
        } catch (e) {
            logError(e)
        }
    };
    imports.wbg.__widl_f_debug_4_ = function(arg0, arg1, arg2, arg3) {
        try {
            console.debug(getObject(arg0), getObject(arg1), getObject(arg2), getObject(arg3));
        } catch (e) {
            logError(e)
        }
    };
    imports.wbg.__widl_f_error_1_ = function(arg0) {
        try {
            console.error(getObject(arg0));
        } catch (e) {
            logError(e)
        }
    };
    imports.wbg.__widl_f_error_4_ = function(arg0, arg1, arg2, arg3) {
        try {
            console.error(getObject(arg0), getObject(arg1), getObject(arg2), getObject(arg3));
        } catch (e) {
            logError(e)
        }
    };
    imports.wbg.__widl_f_info_4_ = function(arg0, arg1, arg2, arg3) {
        try {
            console.info(getObject(arg0), getObject(arg1), getObject(arg2), getObject(arg3));
        } catch (e) {
            logError(e)
        }
    };
    imports.wbg.__widl_f_log_4_ = function(arg0, arg1, arg2, arg3) {
        try {
            console.log(getObject(arg0), getObject(arg1), getObject(arg2), getObject(arg3));
        } catch (e) {
            logError(e)
        }
    };
    imports.wbg.__widl_f_warn_4_ = function(arg0, arg1, arg2, arg3) {
        try {
            console.warn(getObject(arg0), getObject(arg1), getObject(arg2), getObject(arg3));
        } catch (e) {
            logError(e)
        }
    };
    imports.wbg.__wbg_newnoargs_c4b2cbbd30e2d057 = function(arg0, arg1) {
        try {
            var ret = new Function(getStringFromWasm0(arg0, arg1));
            return addHeapObject(ret);
        } catch (e) {
            logError(e)
        }
    };
    imports.wbg.__wbg_call_12b949cfc461d154 = function(arg0, arg1) {
        try {
            try {
                var ret = getObject(arg0).call(getObject(arg1));
                return addHeapObject(ret);
            } catch (e) {
                handleError(e)
            }
        } catch (e) {
            logError(e)
        }
    };
    imports.wbg.__wbg_call_ce7cf17fc6380443 = function(arg0, arg1, arg2) {
        try {
            try {
                var ret = getObject(arg0).call(getObject(arg1), getObject(arg2));
                return addHeapObject(ret);
            } catch (e) {
                handleError(e)
            }
        } catch (e) {
            logError(e)
        }
    };
    imports.wbg.__wbg_new_7dd9b384a913884d = function() {
        try {
            var ret = new Object();
            return addHeapObject(ret);
        } catch (e) {
            logError(e)
        }
    };
    imports.wbg.__wbg_new_d3eff62d5c013634 = function(arg0, arg1) {
        try {
            try {
                var state0 = {a: arg0, b: arg1};
                var cb0 = (arg0, arg1) => {
                    const a = state0.a;
                    state0.a = 0;
                    try {
                        return __wbg_adapter_263(a, state0.b, arg0, arg1);
                    } finally {
                        state0.a = a;
                    }
                };
                var ret = new Promise(cb0);
                return addHeapObject(ret);
            } finally {
                state0.a = state0.b = 0;
            }
        } catch (e) {
            logError(e)
        }
    };
    imports.wbg.__wbg_resolve_6885947099a907d3 = function(arg0) {
        try {
            var ret = Promise.resolve(getObject(arg0));
            return addHeapObject(ret);
        } catch (e) {
            logError(e)
        }
    };
    imports.wbg.__wbg_then_b6fef331fde5cf0a = function(arg0, arg1) {
        try {
            var ret = getObject(arg0).then(getObject(arg1));
            return addHeapObject(ret);
        } catch (e) {
            logError(e)
        }
    };
    imports.wbg.__wbg_then_7d828a330efec051 = function(arg0, arg1, arg2) {
        try {
            var ret = getObject(arg0).then(getObject(arg1), getObject(arg2));
            return addHeapObject(ret);
        } catch (e) {
            logError(e)
        }
    };
    imports.wbg.__wbg_globalThis_22e06d4bea0084e3 = function() {
        try {
            try {
                var ret = globalThis.globalThis;
                return addHeapObject(ret);
            } catch (e) {
                handleError(e)
            }
        } catch (e) {
            logError(e)
        }
    };
    imports.wbg.__wbg_self_00b0599bca667294 = function() {
        try {
            try {
                var ret = self.self;
                return addHeapObject(ret);
            } catch (e) {
                handleError(e)
            }
        } catch (e) {
            logError(e)
        }
    };
    imports.wbg.__wbg_window_aa795c5aad79b8ac = function() {
        try {
            try {
                var ret = window.window;
                return addHeapObject(ret);
            } catch (e) {
                handleError(e)
            }
        } catch (e) {
            logError(e)
        }
    };
    imports.wbg.__wbg_global_cc239dc2303f417c = function() {
        try {
            try {
                var ret = global.global;
                return addHeapObject(ret);
            } catch (e) {
                handleError(e)
            }
        } catch (e) {
            logError(e)
        }
    };
    imports.wbg.__wbg_new_2f80ca95bc180a3c = function(arg0) {
        try {
            var ret = new Float32Array(getObject(arg0));
            return addHeapObject(ret);
        } catch (e) {
            logError(e)
        }
    };
    imports.wbg.__wbg_newwithbyteoffsetandlength_07654e7af606fce0 = function(arg0, arg1, arg2) {
        try {
            var ret = new Float32Array(getObject(arg0), arg1 >>> 0, arg2 >>> 0);
            return addHeapObject(ret);
        } catch (e) {
            logError(e)
        }
    };
    imports.wbg.__wbg_length_4505c57c216b6917 = function(arg0) {
        try {
            var ret = getObject(arg0).length;
            _assertNum(ret);
            return ret;
        } catch (e) {
            logError(e)
        }
    };
    imports.wbg.__wbg_set_0e4bea19d9b9d783 = function(arg0, arg1, arg2) {
        try {
            getObject(arg0).set(getObject(arg1), arg2 >>> 0);
        } catch (e) {
            logError(e)
        }
    };
    imports.wbg.__wbg_buffer_1bb127df6348017b = function(arg0) {
        try {
            var ret = getObject(arg0).buffer;
            return addHeapObject(ret);
        } catch (e) {
            logError(e)
        }
    };
    imports.wbg.__wbg_set_8d5fd23e838df6b0 = function(arg0, arg1, arg2) {
        try {
            try {
                var ret = Reflect.set(getObject(arg0), getObject(arg1), getObject(arg2));
                _assertBoolean(ret);
                return ret;
            } catch (e) {
                handleError(e)
            }
        } catch (e) {
            logError(e)
        }
    };
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
    imports.wbg.__wbindgen_rethrow = function(arg0) {
        throw takeObject(arg0);
    };
    imports.wbg.__wbindgen_memory = function() {
        var ret = wasm.memory;
        return addHeapObject(ret);
    };
    imports.wbg.__wbindgen_closure_wrapper3253 = function(arg0, arg1, arg2) {
        try {

            const state = { a: arg0, b: arg1, cnt: 1 };
            const real = (arg0) => {
                state.cnt++;
                const a = state.a;
                state.a = 0;
                try {
                    return __wbg_adapter_34(a, state.b, arg0);
                } finally {
                    if (--state.cnt === 0) wasm.__wbindgen_export_2.get(93)(a, state.b);
                    else state.a = a;
                }
            }
            ;
            real.original = state;
            var ret = real;
            return addHeapObject(ret);
        } catch (e) {
            logError(e)
        }
    };
    imports.wbg.__wbindgen_closure_wrapper4423 = function(arg0, arg1, arg2) {
        try {

            const state = { a: arg0, b: arg1, cnt: 1 };
            const real = () => {
                state.cnt++;
                const a = state.a;
                state.a = 0;
                try {
                    return __wbg_adapter_28(a, state.b, );
                } finally {
                    if (--state.cnt === 0) wasm.__wbindgen_export_2.get(139)(a, state.b);
                    else state.a = a;
                }
            }
            ;
            real.original = state;
            var ret = real;
            return addHeapObject(ret);
        } catch (e) {
            logError(e)
        }
    };
    imports.wbg.__wbindgen_closure_wrapper3251 = function(arg0, arg1, arg2) {
        try {

            const state = { a: arg0, b: arg1, cnt: 1 };
            const real = (arg0) => {
                state.cnt++;
                const a = state.a;
                state.a = 0;
                try {
                    return __wbg_adapter_31(a, state.b, arg0);
                } finally {
                    if (--state.cnt === 0) wasm.__wbindgen_export_2.get(95)(a, state.b);
                    else state.a = a;
                }
            }
            ;
            real.original = state;
            var ret = real;
            return addHeapObject(ret);
        } catch (e) {
            logError(e)
        }
    };
    imports.wbg.__wbindgen_closure_wrapper6071 = function(arg0, arg1, arg2) {
        try {

            const state = { a: arg0, b: arg1, cnt: 1 };
            const real = (arg0) => {
                state.cnt++;
                const a = state.a;
                state.a = 0;
                try {
                    return __wbg_adapter_37(a, state.b, arg0);
                } finally {
                    if (--state.cnt === 0) wasm.__wbindgen_export_2.get(182)(a, state.b);
                    else state.a = a;
                }
            }
            ;
            real.original = state;
            var ret = real;
            return addHeapObject(ret);
        } catch (e) {
            logError(e)
        }
    };

    if ((typeof URL === 'function' && module instanceof URL) || typeof module === 'string' || (typeof Request === 'function' && module instanceof Request)) {

        const response = fetch(module);
        if (typeof WebAssembly.instantiateStreaming === 'function') {
            result = WebAssembly.instantiateStreaming(response, imports)
            .catch(e => {
                return response
                .then(r => {
                    if (r.headers.get('Content-Type') != 'application/wasm') {
                        console.warn("`WebAssembly.instantiateStreaming` failed because your server does not serve wasm with `application/wasm` MIME type. Falling back to `WebAssembly.instantiate` which is slower. Original error:\n", e);
                        return r.arrayBuffer();
                    } else {
                        throw e;
                    }
                })
                .then(bytes => WebAssembly.instantiate(bytes, imports));
            });
        } else {
            result = response
            .then(r => r.arrayBuffer())
            .then(bytes => WebAssembly.instantiate(bytes, imports));
        }
    } else {

        result = WebAssembly.instantiate(module, imports)
        .then(result => {
            if (result instanceof WebAssembly.Instance) {
                return { instance: result, module };
            } else {
                return result;
            }
        });
    }
    return result.then(({instance, module}) => {
        wasm = instance.exports;
        init.__wbindgen_wasm_module = module;

        return wasm;
    });
}

export default init;

