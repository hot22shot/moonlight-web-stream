import { ByteBuffer } from "./buffer.js"
import { trySendChannel } from "./input.js"

export type TouchConfig = {
    enabled: boolean
}

export class StreamTouch {
    private peer: RTCPeerConnection

    private buffer: ByteBuffer

    private config: TouchConfig
    // TODO: split this into mouse button and mouse move channel so the move channel can be unreliable
    private channel: RTCDataChannel | null

    private supported: boolean | null = null

    constructor(peer: RTCPeerConnection, buffer?: ByteBuffer) {
        this.peer = peer

        this.buffer = buffer ?? new ByteBuffer(1024, false)
        if (this.buffer.isLittleEndian()) {
            throw "invalid buffer endianness"
        }

        this.config = {
            enabled: true,
        }
        this.channel = this.createChannel(this.config)
    }

    setConfig(config: TouchConfig) {
        this.channel?.close()
        this.channel = this.createChannel(config)
        // this.channel?.onmessage = this.onMessage.bind(this)
    }
    private createChannel(config: TouchConfig): RTCDataChannel | null {
        this.config = config
        if (!config.enabled) {
            return null
        }
        const dataChannel = this.peer.createDataChannel("touch")

        return dataChannel
    }

    onTouchStart(event: TouchEvent) {
        for (const touch of event.changedTouches) {
            this.sendTouch(0, touch)
        }
    }
    onTouchMove(event: TouchEvent) {
        for (const touch of event.changedTouches) {
            this.sendTouch(1, touch)
        }
    }
    onTouchEnd(event: TouchEvent) {
        for (const touch of event.changedTouches) {
            this.sendTouch(2, touch)
        }
    }

    private onMessage(event: MessageEvent) {
        const data = event.data
        const buffer = new ByteBuffer(data)
        this.supported = buffer.getBool()
    }

    private sendTouch(type: number, touch: Touch) {
        this.buffer.reset()

        this.buffer.putU8(type)

        this.buffer.putU32(touch.identifier)
        // TODO: find out correct position value
        this.buffer.putF32(touch.clientX / 1000)
        this.buffer.putF32(touch.clientY / 1000)

        this.buffer.putF32(touch.force)

        this.buffer.putF32(touch.radiusX)
        this.buffer.putF32(touch.radiusY)
        this.buffer.putU16(touch.rotationAngle)

        trySendChannel(this.channel, this.buffer)
    }

    isSupported(): boolean | null {
        return this.supported
    }
}
