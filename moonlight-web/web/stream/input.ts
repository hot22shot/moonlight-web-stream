import { ByteBuffer } from "./buffer.js"
import { StreamKeyboard } from "./keyboard.js"
import { StreamMouse } from "./mouse.js"


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

    private buffer: ByteBuffer = new ByteBuffer(1024)

    private keyboard: StreamKeyboard
    private mouse: StreamMouse

    constructor(peer: RTCPeerConnection) {
        this.peer = peer

        this.keyboard = new StreamKeyboard(peer, this.buffer)
        this.mouse = new StreamMouse(peer, this.buffer)
    }

    getKeyboard(): StreamKeyboard {
        return this.keyboard
    }

    getMouse(): StreamMouse {
        return this.mouse
    }

}