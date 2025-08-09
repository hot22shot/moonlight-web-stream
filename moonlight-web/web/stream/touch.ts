import { ByteBuffer } from "./buffer"
import { trySendChannel } from "./input"

export type TouchConfig = {
    enabled: boolean
}

export class StreamMouse {
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
        // this.sendTouch(true, event.tou)
    }
    onTouchEnd(event: TouchEvent) {
        // this.sendTouch(false, event)
    }
    onTouchMove(event: TouchEvent) {
        // this.sendTouch(false, event)
    }

    private onMessage(event: MessageEvent) {
        const data = event.data
        const buffer = new ByteBuffer(data)
    }

    private sendTouch(isDown: boolean, event: Touch) {
        this.buffer.reset()

        this.buffer.putU8(1) // TODO: remove this for two channels
        this.buffer.putBool(isDown)

        trySendChannel(this.channel, this.buffer)
    }

    isSupported(): boolean | null {
        return this.supported
    }
}
