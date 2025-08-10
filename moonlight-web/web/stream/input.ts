import { ByteBuffer } from "./buffer.js"
import { convertToKey, convertToModifiers } from "./keyboard.js"
import { convertToButton } from "./mouse.js"

function trySendChannel(channel: RTCDataChannel | null, buffer: ByteBuffer) {
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

export type StreamInputConfig = {
    keyboardOrdered: boolean
}

export class StreamInput {

    private peer: RTCPeerConnection

    private buffer: ByteBuffer = new ByteBuffer(1024)

    private config: StreamInputConfig

    private keyboard: RTCDataChannel | null = null
    private mouse: RTCDataChannel | null = null
    private touch: RTCDataChannel | null = null

    private touchSupported: boolean | null = null

    constructor(peer: RTCPeerConnection, config?: StreamInputConfig) {
        this.peer = peer

        this.config = config ?? {
            keyboardOrdered: true
        }

        this.createChannels({
            keyboardOrdered: true
        })
    }

    private createChannels(config: StreamInputConfig) {
        this.config = config

        this.keyboard = this.peer.createDataChannel("keyboard", {
            ordered: config.keyboardOrdered
        })

        this.mouse = this.peer.createDataChannel("mouse", {
        })

        this.touch = this.peer.createDataChannel("touch")
        this.touch.onmessage = this.onTouchMessage.bind(this)
    }

    getConfig(): StreamInputConfig {
        return this.config
    }

    // -- Keyboard
    onKeyDown(event: KeyboardEvent) {
        this.sendKeyEvent(true, event)
    }
    onKeyUp(event: KeyboardEvent) {
        this.sendKeyEvent(false, event)
    }
    private sendKeyEvent(isDown: boolean, event: KeyboardEvent) {
        this.buffer.reset()

        const key = convertToKey(event)
        if (!key) {
            return
        }
        const modifiers = convertToModifiers(event)

        this.sendKey(isDown, key, modifiers)
    }

    // Note: key = StreamKeys.VK_, modifiers = StreamKeyModifiers.
    sendKey(isDown: boolean, key: number, modifiers: number) {
        this.buffer.putU8(0)

        this.buffer.putBool(isDown)
        this.buffer.putU8(modifiers)
        this.buffer.putU16(key)

        trySendChannel(this.keyboard, this.buffer)
    }
    sendText(text: string) {
        this.buffer.putU8(1)

        this.buffer.putU8(text.length)
        this.buffer.putUtf8(text)

        trySendChannel(this.keyboard, this.buffer)
    }

    // -- Mouse
    onMouseDown(event: MouseEvent) {
        const button = convertToButton(event)
        if (button == null) {
            return
        }

        this.sendMouseButton(true, button)
    }
    onMouseUp(event: MouseEvent) {
        const button = convertToButton(event)
        if (button == null) {
            return
        }

        this.sendMouseButton(false, button)
    }
    onMouseMove(event: MouseEvent) {
        this.sendMouseMove(event.movementX, event.movementY)
    }
    onWheel(event: WheelEvent) {
        this.sendMouseWheel(event.deltaX, event.deltaY)
    }

    sendMouseMove(movementX: number, movementY: number) {
        this.buffer.reset()

        this.buffer.putU8(0) // TODO: remove this for two channels
        this.buffer.putI16(movementX)
        this.buffer.putI16(movementY)

        trySendChannel(this.mouse, this.buffer)
    }
    // Note: button = StreamMouseButton.
    sendMouseButton(isDown: boolean, button: number) {
        this.buffer.reset()

        this.buffer.putU8(1) // TODO: remove this for two channels
        this.buffer.putBool(isDown)
        this.buffer.putU8(button)

        trySendChannel(this.mouse, this.buffer)
    }
    sendMouseWheel(deltaX: number, deltaY: number) {
        this.buffer.reset()

        this.buffer.putU8(2) // TODO: remove this for two channels
        this.buffer.putI16(deltaX)
        this.buffer.putI16(deltaY)

        trySendChannel(this.mouse, this.buffer)
    }

    // -- Touch
    private onTouchMessage(event: MessageEvent) {
        const data = event.data
        const buffer = new ByteBuffer(data)
        this.touchSupported = buffer.getBool()
    }

    onTouchStart(event: TouchEvent, rect: DOMRect) {
        for (const touch of event.changedTouches) {
            this.sendTouch(0, touch, rect)
        }
    }
    onTouchMove(event: TouchEvent, rect: DOMRect) {
        for (const touch of event.changedTouches) {
            this.sendTouch(1, touch, rect)
        }
    }
    onTouchEnd(event: TouchEvent, rect: DOMRect) {
        for (const touch of event.changedTouches) {
            this.sendTouch(2, touch, rect)
        }
    }

    private sendTouch(type: number, touch: Touch, rect: DOMRect) {
        this.buffer.reset()

        this.buffer.putU8(type)

        this.buffer.putU32(touch.identifier)
        // TODO: find out correct position value
        const x = (touch.clientX - rect.left) / (rect.right - rect.left)
        const y = (touch.clientY - rect.top) / (rect.bottom - rect.top)
        if (x < 0 || x > 1.0 || y < 0 || y > 1.0) {
            // invalid touch
            return
        }
        this.buffer.putF32(x)
        this.buffer.putF32(y)

        this.buffer.putF32(touch.force)

        this.buffer.putF32(touch.radiusX)
        this.buffer.putF32(touch.radiusY)
        this.buffer.putU16(touch.rotationAngle)

        trySendChannel(this.touch, this.buffer)
    }

    isTouchSupported(): boolean | null {
        return this.touchSupported
    }

}