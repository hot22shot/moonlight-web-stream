import { ByteBuffer } from "./buffer.js"
import { StreamKeyboard } from "./keyboard.js"


export function trySendChannel(channel: RTCDataChannel | null, buffer: ByteBuffer) {
    if (!channel || channel.readyState != "open") {
        return
    }

    buffer.flip()
    console.info(buffer)
    const readBuffer = buffer.getReadBuffer()
    if (readBuffer.length == 0) {
        throw "illegal buffer size"
    }
    channel.send(readBuffer.buffer)
}

export class StreamInput {

    private peer: RTCPeerConnection

    private dataBuffer: ByteBuffer = new ByteBuffer(1024)
    private keyboard: StreamKeyboard

    constructor(peer: RTCPeerConnection) {
        this.peer = peer

        this.keyboard = new StreamKeyboard(peer)
    }

    // -- Keyboard
    getKeyboard(): StreamKeyboard {
        return this.keyboard
    }

}