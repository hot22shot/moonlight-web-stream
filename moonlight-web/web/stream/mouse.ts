import { ByteBuffer } from "./buffer.js"
import { StreamMouseButton } from "../api_bindings.js"
import { trySendChannel } from "./input.js"

export type MouseConfig = {
    enabled: boolean
    reliable: boolean
    mode: KeyboardInputMode
}

export type KeyboardInputMode = "relative"

export class StreamMouse {
    private peer: RTCPeerConnection

    private buffer: ByteBuffer

    private config: MouseConfig
    // TODO: split this into mouse button and mouse move channel so the move channel can be unreliable
    private channel: RTCDataChannel | null

    constructor(peer: RTCPeerConnection, buffer?: ByteBuffer) {
        this.peer = peer

        this.buffer = buffer ?? new ByteBuffer(1024, false)
        if (this.buffer.isLittleEndian()) {
            throw "invalid buffer endianness"
        }

        this.config = {
            enabled: true,
            reliable: true,
            mode: "relative",
        }
        this.channel = this.createChannel(this.config)
    }

    setConfig(config: MouseConfig) {
        this.channel?.close()
        this.channel = this.createChannel(config)
    }
    private createChannel(config: MouseConfig): RTCDataChannel | null {
        this.config = config
        if (!config.enabled) {
            return null
        }
        const dataChannel = this.peer.createDataChannel("mouse", {
            maxRetransmits: config.reliable ? undefined : 0
        })

        return dataChannel
    }

    onMouseDown(event: MouseEvent) {
        this.sendMouseButton(true, event)
    }
    onMouseUp(event: MouseEvent) {
        this.sendMouseButton(false, event)
    }
    onMouseMove(event: MouseEvent) {
        this.sendMouseMove(event)
    }

    private sendMouseMove(event: MouseEvent) {
        this.buffer.reset()

        this.buffer.putU8(0) // TODO: remove this for two channels
        this.buffer.putI16(event.movementX)
        this.buffer.putI16(event.movementY)

        trySendChannel(this.channel, this.buffer)
    }
    private sendMouseButton(isDown: boolean, event: MouseEvent) {
        this.buffer.reset()

        const button = convertToButton(event)
        if (button == null) {
            return
        }

        this.buffer.putU8(1) // TODO: remove this for two channels
        this.buffer.putBool(isDown)
        this.buffer.putU8(button)

        trySendChannel(this.channel, this.buffer)
    }
}

const BUTTON_MAPPINGS = new Array(3)
BUTTON_MAPPINGS[0] = StreamMouseButton.LEFT
BUTTON_MAPPINGS[1] = StreamMouseButton.MIDDLE
BUTTON_MAPPINGS[2] = StreamMouseButton.RIGHT

function convertToButton(event: MouseEvent): number | null {
    return BUTTON_MAPPINGS[event.button] ?? null
}