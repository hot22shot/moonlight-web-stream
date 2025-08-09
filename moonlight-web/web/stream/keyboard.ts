import { ByteBuffer } from "./buffer.js"
import { trySendChannel } from "./input.js"
import { StreamKeyModifiers, StreamKeys } from "../api_bindings.js"

export type KeyboardConfig = {
    enabled: boolean
    ordered: boolean
    type: KeyboardInputType
}
export type KeyboardInputType = "updown" | "text"

export class StreamKeyboard {
    private peer: RTCPeerConnection

    private buffer: ByteBuffer

    private config: KeyboardConfig
    private channel: RTCDataChannel | null

    constructor(peer: RTCPeerConnection, buffer?: ByteBuffer) {
        this.peer = peer

        this.buffer = buffer ?? new ByteBuffer(1024, false)
        if (this.buffer.isLittleEndian()) {
            throw "invalid buffer endianness"
        }

        this.config = {
            enabled: true,
            ordered: true,
            type: "updown"
        }
        this.channel = this.createChannel(this.config)
    }

    setConfig(config: KeyboardConfig) {
        this.channel?.close()
        this.channel = this.createChannel(config)
    }
    private createChannel(config: KeyboardConfig): RTCDataChannel | null {
        this.config = config
        if (!config.enabled) {
            return null
        }
        const dataChannel = this.peer.createDataChannel("keyboard", {
            ordered: config.ordered
        })

        return dataChannel
    }

    onKeyDown(event: KeyboardEvent) {
        this.sendKeyEvent(true, event)
    }
    onKeyUp(event: KeyboardEvent) {
        this.sendKeyEvent(false, event)
    }

    private sendKeyEvent(isDown: boolean, event: KeyboardEvent) {
        this.buffer.reset()

        if (this.config.type == "updown") {
            const key = convertToKey(event)
            if (!key) {
                return
            }
            this.buffer.putU8(0)

            let modifiers = convertToModifiers(event)

            this.buffer.putBool(isDown)
            this.buffer.putU8(modifiers)
            this.buffer.putU16(key)
        } else if (this.config.type == "text") {
            const keyText = convertToKeyText(event)
            if (!isDown || !keyText) {
                return
            }
            this.buffer.putU8(1)

            this.buffer.putUtf8(keyText)
        }

        trySendChannel(this.channel, this.buffer)
    }

}

function convertToModifiers(event: KeyboardEvent): number {
    let modifiers = 0;

    if (event.shiftKey) {
        modifiers |= StreamKeyModifiers.MASK_SHIFT;
    }
    if (event.ctrlKey) {
        modifiers |= StreamKeyModifiers.MASK_CTRL;
    }
    if (event.altKey) {
        modifiers |= StreamKeyModifiers.MASK_ALT;
    }
    if (event.metaKey) {
        modifiers |= StreamKeyModifiers.MASK_META;
    }

    return modifiers
}

// WHY: https://developer.mozilla.org/en-US/docs/Web/API/UI_Events/Keyboard_event_code_values
const VK_MAPPINGS: Record<string, number | null> = {
    /* Values on Windows */
    Unidentified: null,
    Escape: StreamKeys.VK_ESCAPE,
    Digit1: StreamKeys.VK_KEY_1,
    Digit2: StreamKeys.VK_KEY_2,
    Digit3: StreamKeys.VK_KEY_3,
    Digit4: StreamKeys.VK_KEY_4,
    Digit5: StreamKeys.VK_KEY_5,
    Digit6: StreamKeys.VK_KEY_6,
    Digit7: StreamKeys.VK_KEY_7,
    Digit8: StreamKeys.VK_KEY_8,
    Digit9: StreamKeys.VK_KEY_9,
    Minus: StreamKeys.VK_OEM_MINUS,
    Equal: null,
    Backspace: StreamKeys.VK_BACK,
    Tab: StreamKeys.VK_TAB,
    KeyQ: StreamKeys.VK_KEY_Q,
    KeyW: StreamKeys.VK_KEY_W,
    KeyE: StreamKeys.VK_KEY_E,
    KeyR: StreamKeys.VK_KEY_R,
    KeyT: StreamKeys.VK_KEY_T,
    KeyY: StreamKeys.VK_KEY_Y,
    KeyU: StreamKeys.VK_KEY_U,
    KeyI: StreamKeys.VK_KEY_I,
    KeyO: StreamKeys.VK_KEY_O,
    KeyP: StreamKeys.VK_KEY_P,
    BracketLeft: null,
    BracketRight: null,
    Enter: StreamKeys.VK_RETURN,
    ControlLeft: StreamKeys.VK_LCONTROL,
    KeyA: StreamKeys.VK_KEY_A,
    KeyS: StreamKeys.VK_KEY_S,
    KeyD: StreamKeys.VK_KEY_D,
    KeyF: StreamKeys.VK_KEY_F,
    KeyG: StreamKeys.VK_KEY_G,
    KeyH: StreamKeys.VK_KEY_H,
    KeyJ: StreamKeys.VK_KEY_J,
    KeyK: StreamKeys.VK_KEY_K,
    KeyL: StreamKeys.VK_KEY_L,
    Semicolon: null,
    Quote: null,
    Backquote: null,
    ShiftLeft: StreamKeys.VK_LSHIFT,
    Backslash: null,
    KeyZ: StreamKeys.VK_KEY_Z,
    KeyX: StreamKeys.VK_KEY_X,
    KeyC: StreamKeys.VK_KEY_C,
    KeyV: StreamKeys.VK_KEY_V,
    KeyB: StreamKeys.VK_KEY_B,
    KeyN: StreamKeys.VK_KEY_N,
    KeyM: StreamKeys.VK_KEY_M,
    Comma: StreamKeys.VK_OEM_COMMA,
    Period: StreamKeys.VK_OEM_PERIOD,
    Slash: null,
    ShiftRight: StreamKeys.VK_RSHIFT,
    NumpadMultiply: StreamKeys.VK_MULTIPLY,
    AltLeft: StreamKeys.VK_LMENU,
    Space: StreamKeys.VK_SPACE,
    CapsLock: null, // TODO
    F1: StreamKeys.VK_F1,
    F2: StreamKeys.VK_F2,
    F3: StreamKeys.VK_F3,
    F4: StreamKeys.VK_F4,
    F5: StreamKeys.VK_F5,
    F6: StreamKeys.VK_F6,
    F7: StreamKeys.VK_F7,
    F8: StreamKeys.VK_F8,
    F9: StreamKeys.VK_F9,
    F10: StreamKeys.VK_F10,
    Pause: StreamKeys.VK_PAUSE,
    ScrollLock: StreamKeys.VK_SCROLL,
    Numpad7: StreamKeys.VK_NUMPAD7,
    Numpad8: StreamKeys.VK_NUMPAD8,
    Numpad9: StreamKeys.VK_NUMPAD9,
    NumpadSubstract: StreamKeys.VK_SUBTRACT,
    Numpad4: StreamKeys.VK_NUMPAD4,
    Numpad5: StreamKeys.VK_NUMPAD5,
    Numpad6: StreamKeys.VK_NUMPAD6,
    NumpadAdd: StreamKeys.VK_ADD,
    Numpad1: StreamKeys.VK_NUMPAD1,
    Numpad2: StreamKeys.VK_NUMPAD2,
    Numpad3: StreamKeys.VK_NUMPAD3,
    Numpad0: StreamKeys.VK_NUMPAD0,
    NumpadDecimal: StreamKeys.VK_DECIMAL,
    PrintScreen: StreamKeys.VK_SNAPSHOT,
    IntlBackslash: null,
    F11: StreamKeys.VK_F11,
    F12: StreamKeys.VK_F12,
    NumpadEqual: null,
    F13: StreamKeys.VK_F13,
    F14: StreamKeys.VK_F14,
    F15: StreamKeys.VK_F15,
    F16: StreamKeys.VK_F16,
    F17: StreamKeys.VK_F17,
    F18: StreamKeys.VK_F18,
    F19: StreamKeys.VK_F19,
    F20: StreamKeys.VK_F20,
    F21: StreamKeys.VK_F21,
    F22: StreamKeys.VK_F22,
    F23: StreamKeys.VK_F23,
    KanaMode: StreamKeys.VK_KANA,
    Lang2: StreamKeys.VK_HANJA,
    Lang1: null, // TODO
    IntlRo: null,
    F24: StreamKeys.VK_F24,
    Lang4: null,
    Lang3: null,
    Convert: StreamKeys.VK_CONVERT,
    NonConvert: StreamKeys.VK_NONCONVERT,
    IntlYen: null,
    NumpadComma: StreamKeys.VK_OEM_COMMA,
    Undo: null,
    Paste: null,
    MediaTrackPrevious: StreamKeys.VK_MEDIA_PREV_TRACK,
    Cut: null,
    MediaTrackNext: StreamKeys.VK_MEDIA_NEXT_TRACK,
    NumpadEnter: StreamKeys.VK_RETURN,
    ControlRight: StreamKeys.VK_RCONTROL,
    LaunchMail: StreamKeys.VK_LAUNCH_MAIL,
    AudioVolumeMute: StreamKeys.VK_VOLUME_MUTE,
    LaunchApp2: StreamKeys.VK_LAUNCH_APP2,
    MediaPlayPause: StreamKeys.VK_MEDIA_PLAY_PAUSE,
    MediaStop: StreamKeys.VK_MEDIA_STOP,
    Eject: null,
    VolumeDown: StreamKeys.VK_VOLUME_DOWN,
    AudioVolumeDown: StreamKeys.VK_VOLUME_DOWN,
    VolumeUp: StreamKeys.VK_VOLUME_UP,
    AudioVolumeUp: StreamKeys.VK_VOLUME_UP,
    BrowserHome: StreamKeys.VK_BROWSER_HOME,
    NumpadDivide: StreamKeys.VK_DIVIDE,
    // PrintScreen: null,
    AltRight: StreamKeys.VK_RMENU,
    Help: StreamKeys.VK_HELP,
    NumLock: StreamKeys.VK_NUMLOCK,
    // Pause: StreamKeys.VK_PAUSE,
    Home: StreamKeys.VK_HOME,
    ArrowUp: StreamKeys.VK_UP,
    PageUp: StreamKeys.VK_PRIOR,
    ArrowLeft: StreamKeys.VK_LEFT,
    ArrowRight: StreamKeys.VK_RIGHT,
    End: StreamKeys.VK_END,
    ArrowDown: StreamKeys.VK_DOWN,
    PageDown: StreamKeys.VK_NEXT,
    Insert: StreamKeys.VK_INSERT,
    Delete: StreamKeys.VK_DELETE,
    MetaLeft: StreamKeys.VK_LWIN,
    OsLeft: StreamKeys.VK_LWIN,
    MetaRight: StreamKeys.VK_RWIN,
    OsRight: StreamKeys.VK_RWIN,
    ContextMenu: null, // TODO
    Power: null,
    Sleep: StreamKeys.VK_SLEEP,
    WakeUp: null,
    BrowserSearch: StreamKeys.VK_BROWSER_SEARCH,
    BrowserFavorites: StreamKeys.VK_BROWSER_FAVORITES,
    BrowserRefresh: StreamKeys.VK_BROWSER_REFRESH,
    BrowserStop: StreamKeys.VK_BROWSER_STOP,
    BrowserForward: StreamKeys.VK_BROWSER_FORWARD,
    BrowserBack: StreamKeys.VK_BROWSER_BACK,
    LaunchApp1: StreamKeys.VK_LAUNCH_APP1,
    // LaunchMail: StreamKeys.VK_LAUNCH_MAIL,
    MediaSelect: StreamKeys.VK_MEDIA_SELECT,
    // Lang2: StreamKeys.VK_HANJA,
    // Lang1: null,
}

function convertToKey(event: KeyboardEvent): number | null {
    return VK_MAPPINGS[event.code]
}

function convertToKeyText(event: KeyboardEvent): string | null {
    return event.key
}