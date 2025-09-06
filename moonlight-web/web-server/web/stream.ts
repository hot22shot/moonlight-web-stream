import { Api, getApi } from "./api.js";
import { Component } from "./component/index.js";
import { showErrorPopup } from "./component/error.js";
import { getStreamerSize, InfoEvent, Stream } from "./stream/index.js"
import { Modal, showMessage, showModal } from "./component/modal/index.js";
import { setSidebar, setSidebarExtended, setSidebarStyle, Sidebar } from "./component/sidebar/index.js";
import { defaultStreamInputConfig, ScreenKeyboardSetVisibleEvent, StreamInputConfig } from "./stream/input.js";
import { defaultStreamSettings, getLocalStreamSettings, StreamSettings } from "./component/settings_menu.js";
import { SelectComponent } from "./component/input.js";
import { getStandardVideoFormats, getSupportedVideoFormats } from "./stream/video.js";
import { StreamCapabilities, StreamKeys } from "./api_bindings.js";
import { ScreenKeyboard, TextEvent } from "./screen_keyboard.js";
import { FormModal } from "./component/modal/form.js";

async function startApp() {
    const api = await getApi()

    const rootElement = document.getElementById("root");
    if (rootElement == null) {
        showErrorPopup("couldn't find root element", true)
        return;
    }

    // Get Host and App via Query
    const queryParams = new URLSearchParams(location.search)

    const hostIdStr = queryParams.get("hostId")
    const appIdStr = queryParams.get("appId")
    if (hostIdStr == null || appIdStr == null) {
        await showMessage("No Host or no App Id found")

        window.close()
        return
    }
    const hostId = Number.parseInt(hostIdStr)
    const appId = Number.parseInt(appIdStr)

    // Start and Mount App
    const app = new ViewerApp(api, hostId, appId)
    app.mount(rootElement)
}

// Prevent starting transition
window.requestAnimationFrame(() => {
    // Note: elements is a live array
    const elements = document.getElementsByClassName("prevent-start-transition")
    while (elements.length > 0) {
        elements.item(0)?.classList.remove("prevent-start-transition")
    }
})

startApp()

class ViewerApp implements Component {
    private api: Api

    private sidebar: ViewerSidebar

    private div = document.createElement("div")
    private videoElement = document.createElement("video")

    private stream: Stream | null = null

    private streamerSize: [number, number]

    constructor(api: Api, hostId: number, appId: number) {
        this.api = api

        // Configure sidebar
        this.sidebar = new ViewerSidebar(this)
        setSidebar(this.sidebar)

        // Configure stream
        const settings = getLocalStreamSettings() ?? defaultStreamSettings()

        let browserWidth = Math.max(document.documentElement.clientWidth || 0, window.innerWidth || 0)
        let browserHeight = Math.max(document.documentElement.clientHeight || 0, window.innerHeight || 0)

        this.startStream(hostId, appId, settings, [browserWidth, browserHeight])

        this.streamerSize = getStreamerSize(settings, [browserWidth, browserHeight])

        // Configure video element
        this.videoElement.classList.add("video-stream")
        this.videoElement.preload = "none"
        this.videoElement.controls = false
        this.videoElement.autoplay = true
        this.videoElement.disablePictureInPicture = true
        this.videoElement.playsInline = true
        this.videoElement.muted = true

        this.div.appendChild(this.videoElement)

        // Configure input
        window.addEventListener("keydown", this.onKeyDown.bind(this), { passive: false })
        window.addEventListener("keyup", this.onKeyUp.bind(this), { passive: false })

        this.div.addEventListener("mousedown", this.onMouseButtonDown.bind(this), { passive: false })
        this.div.addEventListener("mouseup", this.onMouseButtonUp.bind(this), { passive: false })
        this.div.addEventListener("mousemove", this.onMouseMove.bind(this), { passive: false })
        this.div.addEventListener("wheel", this.onMouseWheel.bind(this), { passive: false })

        this.div.addEventListener("touchstart", this.onTouchStart.bind(this), { passive: false })
        this.div.addEventListener("touchend", this.onTouchEnd.bind(this), { passive: false })
        this.div.addEventListener("touchmove", this.onTouchMove.bind(this), { passive: false })

        window.addEventListener("gamepadconnected", this.onGamepadConnect.bind(this))
        window.addEventListener("gamepaddisconnected", this.onGamepadDisconnect.bind(this))
        // Connect all gamepads
        for (const gamepad of navigator.getGamepads()) {
            if (gamepad != null) {
                this.onGamepadAdd(gamepad)
            }
        }
    }

    private async startStream(hostId: number, appId: number, settings: StreamSettings, browserSize: [number, number]) {
        setSidebarStyle({
            edge: settings.sidebarEdge,
        })

        let supportedVideoFormats = getStandardVideoFormats()
        if (settings.dontForceH264) {
            supportedVideoFormats = await getSupportedVideoFormats()
        }

        this.stream = new Stream(this.api, hostId, appId, settings, supportedVideoFormats, browserSize)

        // Add app info listener
        this.stream.addInfoListener(this.onInfo.bind(this))

        // Create connection info modal
        const connectionInfo = new ConnectionInfoModal()
        this.stream.addInfoListener(connectionInfo.onInfo.bind(connectionInfo))
        showModal(connectionInfo)

        // Set video
        this.videoElement.srcObject = this.stream.getMediaStream()

        // Start animation frame loop
        this.onTouchUpdate()
        this.onGamepadUpdate()

        this.stream.getInput().addScreenKeyboardVisibleEvent(this.onScreenKeyboardSetVisible.bind(this))
    }

    private async onInfo(event: InfoEvent) {
        const data = event.detail

        if (data.type == "app") {
            const app = data.app

            document.title = `Stream: ${app.title}`
        } else if (data.type == "connectionComplete") {
            this.sidebar.onCapabilitiesChange(data.capabilities)
        }
    }

    onUserInteraction() {
        this.videoElement.muted = false
    }
    private onScreenKeyboardSetVisible(event: ScreenKeyboardSetVisibleEvent) {
        console.info(event.detail)
        const screenKeyboard = this.sidebar.getScreenKeyboard()

        const newShown = event.detail.visible
        if (newShown != screenKeyboard.isVisible()) {
            if (newShown) {
                screenKeyboard.show()
            } else {
                screenKeyboard.hide()
            }
        }
    }

    // Keyboard
    onKeyDown(event: KeyboardEvent) {
        this.onUserInteraction()

        event.preventDefault()
        this.stream?.getInput().onKeyDown(event)
    }
    onKeyUp(event: KeyboardEvent) {
        this.onUserInteraction()

        event.preventDefault()
        this.stream?.getInput().onKeyUp(event)
    }

    // Mouse
    onMouseButtonDown(event: MouseEvent) {
        this.onUserInteraction()

        event.preventDefault()
        this.stream?.getInput().onMouseDown(event, this.getStreamRect());
    }
    onMouseButtonUp(event: MouseEvent) {
        this.onUserInteraction()

        event.preventDefault()
        this.stream?.getInput().onMouseUp(event)
    }
    onMouseMove(event: MouseEvent) {
        event.preventDefault()
        this.stream?.getInput().onMouseMove(event, this.getStreamRect())
    }
    onMouseWheel(event: WheelEvent) {
        event.preventDefault()
        this.stream?.getInput().onMouseWheel(event)
    }

    // Touch
    onTouchStart(event: TouchEvent) {
        this.onUserInteraction()

        event.preventDefault()
        this.stream?.getInput().onTouchStart(event, this.getStreamRect())
    }
    onTouchEnd(event: TouchEvent) {
        this.onUserInteraction()

        event.preventDefault()
        this.stream?.getInput().onTouchEnd(event, this.getStreamRect())
    }
    onTouchCancel(event: TouchEvent) {
        this.onUserInteraction()

        event?.preventDefault()
        this.stream?.getInput().onTouchCancel(event, this.getStreamRect())
    }
    onTouchUpdate() {
        this.stream?.getInput().onTouchUpdate(this.getStreamRect())

        window.requestAnimationFrame(this.onTouchUpdate.bind(this))
    }
    onTouchMove(event: TouchEvent) {
        event.preventDefault()
        this.stream?.getInput().onTouchMove(event, this.getStreamRect())
    }

    // Gamepad
    onGamepadConnect(event: GamepadEvent) {
        this.onGamepadAdd(event.gamepad)
    }
    onGamepadAdd(gamepad: Gamepad) {
        this.stream?.getInput().onGamepadConnect(gamepad)
    }
    onGamepadDisconnect(event: GamepadEvent) {
        this.stream?.getInput().onGamepadDisconnect(event)
    }
    onGamepadUpdate() {
        this.stream?.getInput().onGamepadUpdate()

        window.requestAnimationFrame(this.onGamepadUpdate.bind(this))
    }

    mount(parent: HTMLElement): void {
        parent.appendChild(this.div)
    }
    unmount(parent: HTMLElement): void {
        parent.removeChild(this.div)
    }

    getStreamRect(): DOMRect {
        // The bounding rect of the videoElement can be bigger than the actual video
        // -> We need to correct for this when sending positions, else positions are wrong

        const videoSize = this.stream?.getStreamerSize() ?? this.streamerSize
        const videoAspect = videoSize[0] / videoSize[1]

        const boundingRect = this.videoElement.getBoundingClientRect()
        const boundingRectAspect = boundingRect.width / boundingRect.height

        let x = boundingRect.x
        let y = boundingRect.y
        let videoMultiplier
        if (boundingRectAspect > videoAspect) {
            // How much is the video scaled up
            videoMultiplier = boundingRect.height / videoSize[1]

            // Note: Both in boundingRect / page scale
            const boundingRectHalfWidth = boundingRect.width / 2
            const videoHalfWidth = videoSize[0] * videoMultiplier / 2

            x += boundingRectHalfWidth - videoHalfWidth
        } else {
            // Same as above but inverted
            videoMultiplier = boundingRect.width / videoSize[0]

            const boundingRectHalfHeight = boundingRect.height / 2
            const videoHalfHeight = videoSize[1] * videoMultiplier / 2

            y += boundingRectHalfHeight - videoHalfHeight
        }

        return new DOMRect(
            x,
            y,
            videoSize[0] * videoMultiplier,
            videoSize[1] * videoMultiplier
        )
    }
    getElement(): HTMLElement {
        return this.videoElement
    }
    getStream(): Stream | null {
        return this.stream
    }
}

class ConnectionInfoModal implements Modal<void> {

    private eventTarget = new EventTarget()

    private root = document.createElement("div")

    private text = document.createElement("p")

    private debugDetailButton = document.createElement("button")
    private debugDetail = "" // We store this seperate because line breaks don't work when the element is not mounted on the dom
    private debugDetailDisplay = document.createElement("div")

    constructor() {
        this.root.classList.add("modal-video-connect")

        this.text.innerText = "Connecting"
        this.root.appendChild(this.text)

        this.debugDetailButton.innerText = "Show Logs"
        this.debugDetailButton.addEventListener("click", this.onDebugDetailClick.bind(this))
        this.root.appendChild(this.debugDetailButton)

        this.debugDetailDisplay.classList.add("textlike")
        this.debugDetailDisplay.classList.add("modal-video-connect-debug")
    }

    private onDebugDetailClick() {
        let debugDetailCurrentlyShown = this.root.contains(this.debugDetailDisplay)

        if (debugDetailCurrentlyShown) {
            this.debugDetailButton.innerText = "Show Logs"
            this.root.removeChild(this.debugDetailDisplay)
        } else {
            this.debugDetailButton.innerText = "Hide Logs"
            this.root.appendChild(this.debugDetailDisplay)
            this.debugDetailDisplay.innerText = this.debugDetail
        }
    }

    private debugLog(line: string) {
        this.debugDetail += `${line}\n`
        this.debugDetailDisplay.innerText = this.debugDetail
        console.info(`[Stream]: ${line}`)
    }

    onInfo(event: InfoEvent) {
        const data = event.detail

        if (data.type == "stageStarting") {
            const text = `Server: Starting Stage: ${data.stage}`
            this.text.innerText = text
            this.debugLog(text)
        } else if (data.type == "stageComplete") {
            const text = `Server: Completed Stage: ${data.stage}`
            this.text.innerText = text
            this.debugLog(text)
        } else if (data.type == "stageFailed") {
            const text = `Server: Failed Stage: ${data.stage} with error ${data.errorCode}`
            this.text.innerText = text
            this.debugLog(text)
        } else if (data.type == "connectionComplete") {
            const text = `Connection Complete`
            this.text.innerText = text
            this.debugLog(text)

            this.eventTarget.dispatchEvent(new Event("ml-connected"))
        } else if (data.type == "addDebugLine") {
            this.debugLog(data.line)
        }
        // Reopen the modal cause we might already be closed at this point
        else if (data.type == "connectionTerminated") {
            const text = `Server: Connection Terminated with code ${data.errorCode}`
            this.text.innerText = text
            this.debugLog(text)

            showModal(this)
        } else if (data.type == "error") {
            const text = `Server: Error: ${data.message}`
            this.text.innerText = text
            this.debugLog(text)

            showModal(this)
        }
    }

    onFinish(abort: AbortSignal): Promise<void> {
        return new Promise((resolve, reject) => {
            this.eventTarget.addEventListener("ml-connected", () => resolve(), { once: true, signal: abort })
        })
    }

    mount(parent: HTMLElement): void {
        parent.appendChild(this.root)
    }
    unmount(parent: HTMLElement): void {
        parent.removeChild(this.root)
    }
}

class ViewerSidebar implements Component, Sidebar {
    private app: ViewerApp

    private div = document.createElement("div")

    private buttonDiv = document.createElement("div")

    private sendKeycodeButton = document.createElement("button")

    private keyboardButton = document.createElement("button")
    private screenKeyboard = new ScreenKeyboard()

    private lockMouseButton = document.createElement("button")
    private fullscreenButton = document.createElement("button")

    private mouseMode: SelectComponent
    private touchMode: SelectComponent

    private config: StreamInputConfig = defaultStreamInputConfig()

    constructor(app: ViewerApp) {
        this.app = app

        // Configure divs
        this.div.classList.add("sidebar-stream")

        this.div.addEventListener("click", this.onStopPropagation.bind(this))
        this.div.addEventListener("keydown", this.onStopPropagation.bind(this))
        this.div.addEventListener("keyup", this.onStopPropagation.bind(this))
        this.div.addEventListener("keypress", this.onStopPropagation.bind(this))
        this.div.addEventListener("touchstart", this.onStopPropagation.bind(this))
        this.div.addEventListener("touchmove", this.onStopPropagation.bind(this))
        this.div.addEventListener("touchend", this.onStopPropagation.bind(this))
        this.div.addEventListener("touchcancel", this.onStopPropagation.bind(this))

        this.buttonDiv.classList.add("sidebar-stream-buttons")
        this.div.appendChild(this.buttonDiv)

        // Send keycode
        this.sendKeycodeButton.innerText = "Send Keycode"
        this.sendKeycodeButton.addEventListener("click", async () => {
            const key = await showModal(new SendKeycodeModal())

            if (key == null) {
                return
            }

            this.app.getStream()?.getInput().sendKey(true, key, 0)
            this.app.getStream()?.getInput().sendKey(false, key, 0)
        })
        this.buttonDiv.appendChild(this.sendKeycodeButton)

        // Pointer Lock
        this.lockMouseButton.innerText = "Lock Mouse"
        this.lockMouseButton.addEventListener("click", async () => {
            setSidebarExtended(false)

            const element = await app.getElement()
            if ("requestPointerLock" in element && typeof element.requestPointerLock == "function") {
                await element.requestPointerLock()
            } else {
                await showMessage("Pointer Lock not Supported")
            }
        })
        this.buttonDiv.appendChild(this.lockMouseButton)

        // Pop up keyboard
        this.keyboardButton.innerText = "Keyboard"
        this.keyboardButton.addEventListener("click", async event => {
            // This could trigger the screen keyboard listeners for detecting close
            event.stopPropagation()

            setSidebarExtended(false)
            this.screenKeyboard.show()
        })
        this.buttonDiv.appendChild(this.keyboardButton)

        this.screenKeyboard.addKeyDownListener(this.onKeyDown.bind(this))
        this.screenKeyboard.addKeyUpListener(this.onKeyUp.bind(this))
        this.screenKeyboard.addTextListener(this.onText.bind(this))
        this.div.appendChild(this.screenKeyboard.getHiddenElement())


        // Fullscreen
        this.fullscreenButton.innerText = "Fullscreen"
        this.fullscreenButton.addEventListener("click", async () => {
            const root = document.getElementById("root")
            if (root) {
                await root.requestFullscreen({
                    navigationUI: "hide"
                })

                try {
                    if (screen && "orientation" in screen) {
                        const orientation = screen.orientation

                        if ("lock" in orientation && typeof orientation.lock == "function") {
                            await orientation.lock("landscape")
                        }
                    }
                } catch (e) {
                    console.warn("failed to set orientation to landscape", e)
                }
            }
        })
        this.buttonDiv.appendChild(this.fullscreenButton)

        // Select Mouse Mode
        this.mouseMode = new SelectComponent("mouseMode", [
            { value: "relative", name: "Relative" },
            { value: "follow", name: "Follow" },
            { value: "pointAndDrag", name: "Point and Drag" }
        ], {
            displayName: "Mouse Mode",
            preSelectedOption: this.config.mouseMode
        })
        this.mouseMode.addChangeListener(this.onMouseModeChange.bind(this))
        this.mouseMode.mount(this.div)

        // Select Touch Mode
        this.touchMode = new SelectComponent("touchMode", [
            { value: "touch", name: "Touch" },
            { value: "mouseRelative", name: "Relative" },
            { value: "pointAndDrag", name: "Point and Drag" }
        ], {
            displayName: "Touch Mode",
            preSelectedOption: this.config.touchMode
        })
        this.touchMode.addChangeListener(this.onTouchModeChange.bind(this))
        this.touchMode.mount(this.div)
    }

    onCapabilitiesChange(capabilities: StreamCapabilities) {
        this.touchMode.setOptionEnabled("touch", capabilities.touch)
    }

    getScreenKeyboard(): ScreenKeyboard {
        return this.screenKeyboard
    }

    // Stop propagation so the stream doesn't get it
    private onStopPropagation(event: Event) {
        event.stopPropagation()
    }

    // -- Keyboard
    private onText(event: TextEvent) {
        this.app.getStream()?.getInput().sendText(event.detail.text)
    }
    private onKeyDown(event: KeyboardEvent) {
        this.app.getStream()?.getInput().onKeyDown(event)
    }
    private onKeyUp(event: KeyboardEvent) {
        this.app.getStream()?.getInput().onKeyUp(event)
    }

    // -- Mouse Mode
    private onMouseModeChange() {
        this.config.mouseMode = this.mouseMode.getValue() as any
        this.app.getStream()?.getInput().setConfig(this.config)
    }

    // -- Touch Mode
    private onTouchModeChange() {
        this.config.touchMode = this.touchMode.getValue() as any
        this.app.getStream()?.getInput().setConfig(this.config)
    }

    extended(): void {

    }
    unextend(): void {

    }

    mount(parent: HTMLElement): void {
        parent.appendChild(this.div)
    }
    unmount(parent: HTMLElement): void {
        parent.removeChild(this.div)
    }
}

class SendKeycodeModal extends FormModal<number> {

    private dropdownSearch: SelectComponent

    constructor() {
        super()

        const keyList = []
        for (const keyName of Object.keys(StreamKeys)) {
            const keyValue = StreamKeys[keyName]

            const PREFIX = "VK_"

            let name = keyName
            if (name.startsWith(PREFIX)) {
                name = name.slice(PREFIX.length)
            }

            keyList.push({
                value: keyValue.toString(),
                name
            })
        }

        this.dropdownSearch = new SelectComponent("winKeycode", keyList, {
            hasSearch: true,
            displayName: "Select Keycode"
        })
    }

    mountForm(form: HTMLFormElement): void {
        form.addEventListener("click", this.onStopPropagation.bind(this))
        form.addEventListener("keydown", this.onStopPropagation.bind(this))
        form.addEventListener("keyup", this.onStopPropagation.bind(this))
        form.addEventListener("keypress", this.onStopPropagation.bind(this))
        form.addEventListener("touchstart", this.onStopPropagation.bind(this))
        form.addEventListener("touchmove", this.onStopPropagation.bind(this))
        form.addEventListener("touchend", this.onStopPropagation.bind(this))
        form.addEventListener("touchcancel", this.onStopPropagation.bind(this))

        this.dropdownSearch.mount(form)
    }

    // Stop propagation so the stream doesn't get it
    private onStopPropagation(event: Event) {
        event.stopPropagation()
    }

    reset(): void {
        this.dropdownSearch.reset()
    }

    submit(): number | null {
        const keyString = this.dropdownSearch.getValue()
        if (keyString == null) {
            return null
        }

        return parseInt(keyString)
    }
}