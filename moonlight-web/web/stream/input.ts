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

        console.info(this.dataBuffer)
        this.dataBuffer.flip()
        const buffer = this.dataBuffer.getReadBuffer()
        if (buffer.length == 0) {
            throw "illegal buffer size"
        }
        channel.send(buffer.buffer)
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
        this.sendKeyEvent(true, event)
    }
    onKeyUp(event: KeyboardEvent) {
        this.sendKeyEvent(false, event)
    }

    private sendKeyEvent(isDown: boolean, event: KeyboardEvent) {
        this.dataBuffer.reset()

        if (this.keyboardConfig.type == "updown") {
            this.dataBuffer.putU8(0)
            this.dataBuffer.putBool(isDown)
        } else if (this.keyboardConfig.type == "text") {
            if (!isDown) {
                return
            }

            this.dataBuffer.putU8(1)
            this.dataBuffer.putUtf8(event.key)
        }

        this.trySendChannel(this.keyboardChannel)
    }
}