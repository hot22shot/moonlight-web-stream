import { StreamMouseButton } from "../api_bindings.js"
import { ByteBuffer } from "./buffer.js"
import { convertToKey, convertToModifiers } from "./keyboard.js"
import { convertToButton } from "./mouse.js"

const TOUCH_AS_CLICK_MAX_DISTANCE = 30
const TOUCH_AS_CLICK_MAX_TIME_MS = 300

function trySendChannel(channel: RTCDataChannel | null, buffer: ByteBuffer) {
    if (!channel || channel.readyState != "open") {
        return
    }

    buffer.flip()
    const readBuffer = buffer.getReadBuffer()
    if (readBuffer.length == 0) {
        throw "illegal buffer size"
    }
    channel.send(readBuffer.buffer)
}

export type StreamInputConfig = {
    keyboardOrdered: boolean
    mouseMode: "relative" | "pointAndDrag"
    touchMode: "touch" | "mouseRelative" | "pointAndDrag"
}

export const DEFAULT_STREAM_INPUT_CONFIG: StreamInputConfig = {
    keyboardOrdered: true,
    mouseMode: "pointAndDrag",
    touchMode: "pointAndDrag"
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

        this.config = config ?? DEFAULT_STREAM_INPUT_CONFIG

        this.createChannels(this.config)
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

    setConfig(config: StreamInputConfig) {
        this.config = config
        this.createChannels(config)

        // Touch
        this.primaryTouch = null
        this.touchTracker.clear()
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
    onMouseDown(event: MouseEvent, rect: DOMRect) {
        const button = convertToButton(event)
        if (button == null) {
            return
        }

        if (this.config.mouseMode == "relative") {
            this.sendMouseButton(true, button)
        } else if (this.config.mouseMode == "pointAndDrag") {
            this.sendMousePositionClientCoordinates(event.clientX, event.clientY, rect, button)
        }
    }
    onMouseUp(event: MouseEvent) {
        const button = convertToButton(event)
        if (button == null) {
            return
        }

        this.sendMouseButton(false, button)
    }
    onMouseMove(event: MouseEvent) {
        if (this.config.mouseMode == "relative") {
            this.sendMouseMove(event.movementX, event.movementY)
        } else if (this.config.mouseMode == "pointAndDrag") {
            if (event.buttons) {
                // some button pressed
                this.sendMouseMove(event.movementX, event.movementY)
            }
        }
    }
    onWheel(event: WheelEvent) {
        this.sendMouseWheel(event.deltaX, event.deltaY)
    }

    sendMouseMove(movementX: number, movementY: number) {
        this.buffer.reset()

        this.buffer.putU8(0)
        this.buffer.putI16(movementX)
        this.buffer.putI16(movementY)

        trySendChannel(this.mouse, this.buffer)
    }
    sendMousePosition(x: number, y: number, referenceWidth: number, referenceHeight: number) {
        this.buffer.reset()

        this.buffer.putU8(1)
        this.buffer.putI16(x)
        this.buffer.putI16(y)
        this.buffer.putI16(referenceWidth)
        this.buffer.putI16(referenceHeight)

        trySendChannel(this.mouse, this.buffer)
    }
    sendMousePositionClientCoordinates(clientX: number, clientY: number, rect: DOMRect, mouseButton?: number) {
        const position = this.calcNormalizedPosition(clientX, clientY, rect)
        if (position) {
            const [x, y] = position
            this.sendMousePosition(x * 4096.0, y * 4096.0, 4096.0, 4096.0)

            if (mouseButton != undefined) {
                this.sendMouseButton(true, mouseButton)
            }
        }
    }
    // Note: button = StreamMouseButton.
    sendMouseButton(isDown: boolean, button: number) {
        this.buffer.reset()

        this.buffer.putU8(2)
        this.buffer.putBool(isDown)
        this.buffer.putU8(button)

        trySendChannel(this.mouse, this.buffer)
    }
    sendMouseWheel(deltaX: number, deltaY: number) {
        this.buffer.reset()

        this.buffer.putU8(3)
        this.buffer.putI16(deltaX)
        this.buffer.putI16(deltaY)

        trySendChannel(this.mouse, this.buffer)
    }

    // -- Touch
    private touchTracker: Map<number, {
        startTime: number
        originX: number
        originY: number
        x: number
        y: number
    }> = new Map()
    private primaryTouch: number | null = null

    private onTouchMessage(event: MessageEvent) {
        const data = event.data
        const buffer = new ByteBuffer(data)
        this.touchSupported = buffer.getBool()
    }

    private updateTouchTracker(touch: Touch) {
        const oldTouch = this.touchTracker.get(touch.identifier)
        if (!oldTouch) {
            this.touchTracker.set(touch.identifier, {
                startTime: Date.now(),
                originX: touch.clientX,
                originY: touch.clientY,
                x: touch.clientX,
                y: touch.clientY
            })
        } else {
            oldTouch.x = touch.clientX
            oldTouch.y = touch.clientY
        }
    }

    onTouchStart(event: TouchEvent, rect: DOMRect) {
        for (const touch of event.changedTouches) {
            this.updateTouchTracker(touch)
        }

        if (this.config.touchMode == "touch") {
            for (const touch of event.changedTouches) {
                this.sendTouch(0, touch, rect)
            }
        } else if (this.config.touchMode == "mouseRelative" || this.config.touchMode == "pointAndDrag") {
            const touch = event.changedTouches[0]

            if (this.primaryTouch == null && touch) {
                this.primaryTouch = touch.identifier

                if (this.config.touchMode == "pointAndDrag") {
                    this.sendMousePositionClientCoordinates(touch.clientX, touch.clientY, rect, StreamMouseButton.LEFT)
                }
            }
        }
    }
    onTouchMove(event: TouchEvent, rect: DOMRect) {
        if (this.config.touchMode == "touch") {
            for (const touch of event.changedTouches) {
                this.sendTouch(1, touch, rect)
            }
        } else if (this.config.touchMode == "mouseRelative" || this.config.touchMode == "pointAndDrag") {
            for (const touch of event.changedTouches) {
                if (this.primaryTouch != touch.identifier) {
                    continue
                }
                const oldTouch = this.touchTracker.get(this.primaryTouch)
                if (!oldTouch) {
                    continue
                }

                // mouse move
                const movementX = touch.clientX - oldTouch.x;
                const movementY = touch.clientY - oldTouch.y;

                this.sendMouseMove(movementX, movementY)
            }
        }

        for (const touch of event.changedTouches) {
            this.updateTouchTracker(touch)
        }
    }

    onTouchEnd(event: TouchEvent, rect: DOMRect) {
        if (this.config.touchMode == "touch") {
            for (const touch of event.changedTouches) {
                this.sendTouch(2, touch, rect)
            }
        } else if (this.config.touchMode == "mouseRelative" || this.config.touchMode == "pointAndDrag") {
            for (const touch of event.changedTouches) {
                if (this.primaryTouch != touch.identifier) {
                    continue
                }
                const oldTouch = this.touchTracker.get(this.primaryTouch)
                this.primaryTouch = null

                if (this.config.touchMode == "mouseRelative") {
                    // mouse click
                    if (oldTouch
                        && Date.now() - oldTouch.startTime <= TOUCH_AS_CLICK_MAX_TIME_MS
                        && Math.hypot(touch.clientX - oldTouch.originX, touch.clientY - oldTouch.originY) <= TOUCH_AS_CLICK_MAX_DISTANCE
                    ) {
                        this.sendMouseButton(true, StreamMouseButton.LEFT)
                        this.sendMouseButton(false, StreamMouseButton.LEFT)
                    }
                } else if (this.config.touchMode == "pointAndDrag") {
                    this.sendMouseButton(false, StreamMouseButton.LEFT)
                }
            }
        }

        for (const touch of event.changedTouches) {
            this.touchTracker.delete(touch.identifier)
        }
    }

    private calcNormalizedPosition(clientX: number, clientY: number, rect: DOMRect): [number, number] | null {
        const x = (clientX - rect.left) / rect.width
        const y = (clientY - rect.top) / rect.height
        console.info("TOUCH", x, y, rect)
        if (x < 0 || x > 1.0 || y < 0 || y > 1.0) {
            // invalid touch
            return null
        }
        return [x, y]
    }
    private sendTouch(type: number, touch: Touch, rect: DOMRect) {
        this.buffer.reset()

        this.buffer.putU8(type)

        this.buffer.putU32(touch.identifier)

        const position = this.calcNormalizedPosition(touch.clientX, touch.clientY, rect)
        if (!position) {
            return
        }
        const [x, y] = position
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