import { ByteBuffer } from "./buffer.js"

export type KeyboardConfig = {
    enabled: boolean
    ordered: boolean
    type: KeyboardInputType
}
export type KeyboardInputType = "updown" | "text"

export class StreamInput {

    private peer: RTCPeerConnection

    private dataBuffer: ByteBuffer = new ByteBuffer(1024)

    private keyboardConfig: KeyboardConfig
    private keyboardChannel: RTCDataChannel | null

    constructor(peer: RTCPeerConnection) {
        this.peer = peer

        this.keyboardConfig = {
            enabled: true,
            ordered: true,
            type: "text"
        }
        this.keyboardChannel = this.createKeyboardChannel(this.keyboardConfig)
    }

    private trySendChannel(channel: RTCDataChannel | null) {
        if (!channel || channel.readyState != "open") {
            return
        }

        console.info(`SENDING TO ${channel.label}`)
        channel.send(this.dataBuffer.getReadBuffer())
        channel.send("TEST")
    }

    private onError(error: RTCErrorEvent) {
        console.error("RTC Data Channel Error", error)
    }

    // -- Keyboard
    setKeyboardConfig(config: KeyboardConfig) {
        this.keyboardChannel?.close()
        this.keyboardChannel = this.createKeyboardChannel(config)
    }
    private createKeyboardChannel(config: KeyboardConfig): RTCDataChannel | null {
        this.keyboardConfig = config
        if (!config.enabled) {
            return null
        }
        const dataChannel = this.peer.createDataChannel("keyboard", {
            ordered: config.ordered
        })
        dataChannel.onerror = this.onError.bind(this)

        return dataChannel
    }

    onKeyDown(event: KeyboardEvent) {
        this.dataBuffer.reset()
        this.encodeKeyEvent(true, event)

        this.trySendChannel(this.keyboardChannel)
    }
    onKeyUp(event: KeyboardEvent) {
        this.dataBuffer.reset()
        this.encodeKeyEvent(false, event)

        this.trySendChannel(this.keyboardChannel)
    }

    private encodeKeyEvent(isDown: boolean, event: KeyboardEvent) {
        if (this.keyboardConfig.type == "updown") {
            this.dataBuffer.putU8(0)
            this.dataBuffer.putBool(isDown)

        } else if (this.keyboardConfig.type == "text") {
            this.dataBuffer.putU8(1)
            this.dataBuffer.putUtf8(event.key)
        }
    }
}