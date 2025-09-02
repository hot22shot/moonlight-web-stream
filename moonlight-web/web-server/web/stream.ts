import { Api, getApi } from "./api.js";
import { Component } from "./component/index.js";
import { showErrorPopup } from "./component/error.js";
import { InfoEvent, Stream } from "./stream/index.js"
import { Modal, showMessage, showModal } from "./component/modal/index.js";
import { setSidebar, setSidebarExtended, setSidebarStyle, Sidebar } from "./component/sidebar/index.js";
import { defaultStreamInputConfig, ScreenKeyboardSetVisibleEvent, StreamInputConfig } from "./stream/input.js";
import { defaultStreamSettings, getLocalStreamSettings } from "./component/settings_menu.js";
import { SelectComponent } from "./component/input.js";
import { getStandardVideoFormats, getSupportedVideoFormats, VideoCodecSupport } from "./stream/video.js";
import { StreamCapabilities } from "./api_bindings.js";
import { ScreenKeyboard, TextEvent } from "./screen_keyboard.js";

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
    private videoElement = document.createElement("video")

    private stream: Stream | null = null

    constructor(api: Api, hostId: number, appId: number) {
        this.api = api

        // Configure sidebar
        this.sidebar = new ViewerSidebar(this)
        setSidebar(this.sidebar)

        // Configure stream
        this.startStream(hostId, appId)

        // Configure video element
        this.videoElement.classList.add("video-stream")
        this.videoElement.preload = "none"
        this.videoElement.controls = false
        this.videoElement.autoplay = true
        this.videoElement.disablePictureInPicture = true
        this.videoElement.playsInline = true
        this.videoElement.muted = true

        // Configure input
        document.addEventListener("keydown", this.onKeyDown.bind(this), { passive: false })
        document.addEventListener("keyup", this.onKeyUp.bind(this), { passive: false })

        document.addEventListener("mousedown", this.onMouseButtonDown.bind(this), { passive: false })
        document.addEventListener("mouseup", this.onMouseButtonUp.bind(this), { passive: false })
        document.addEventListener("mousemove", this.onMouseMove.bind(this), { passive: false })
        document.addEventListener("wheel", this.onMouseWheel.bind(this), { passive: false })

        document.addEventListener("touchstart", this.onTouchStart.bind(this), { passive: false })
        document.addEventListener("touchend", this.onTouchEnd.bind(this), { passive: false })
        document.addEventListener("touchmove", this.onTouchMove.bind(this), { passive: false })

        window.addEventListener("gamepadconnected", this.onGamepadConnect.bind(this))
        window.addEventListener("gamepaddisconnected", this.onGamepadDisconnect.bind(this))
        // Connect all gamepads
        for (const gamepad of navigator.getGamepads()) {
            if (gamepad != null) {
                this.onGamepadAdd(gamepad)
            }
        }
    }

    private async startStream(hostId: number, appId: number) {
        let viewerWidth = Math.max(document.documentElement.clientWidth || 0, window.innerWidth || 0)
        let viewerHeight = Math.max(document.documentElement.clientHeight || 0, window.innerHeight || 0)

        const settings = getLocalStreamSettings() ?? defaultStreamSettings()
        setSidebarStyle({
            edge: settings.sidebarEdge,
        })

        let supportedVideoFormats = getStandardVideoFormats()
        if (settings.dontForceH264) {
            supportedVideoFormats = await getSupportedVideoFormats()
        }

        this.stream = new Stream(this.api, hostId, appId, settings, supportedVideoFormats, [viewerWidth, viewerHeight])

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
        this.stream?.getInput().onMouseDown(event, this.videoElement.getBoundingClientRect());
    }
    onMouseButtonUp(event: MouseEvent) {
        this.onUserInteraction()

        event.preventDefault()
        this.stream?.getInput().onMouseUp(event)
    }
    onMouseMove(event: MouseEvent) {
        event.preventDefault()
        this.stream?.getInput().onMouseMove(event, this.videoElement.getBoundingClientRect())
    }
    onMouseWheel(event: WheelEvent) {
        event.preventDefault()
        this.stream?.getInput().onMouseWheel(event)
    }

    // Touch
    onTouchStart(event: TouchEvent) {
        this.onUserInteraction()

        event.preventDefault()
        this.stream?.getInput().onTouchStart(event, this.videoElement.getBoundingClientRect())
    }
    onTouchEnd(event: TouchEvent) {
        this.onUserInteraction()

        event.preventDefault()
        this.stream?.getInput().onTouchEnd(event, this.videoElement.getBoundingClientRect())
    }
    onTouchUpdate() {
        this.stream?.getInput().onTouchUpdate(this.videoElement.getBoundingClientRect())

        window.requestAnimationFrame(this.onTouchUpdate.bind(this))
    }
    onTouchMove(event: TouchEvent) {
        event.preventDefault()
        this.stream?.getInput().onTouchMove(event, this.videoElement.getBoundingClientRect())
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
        parent.appendChild(this.videoElement)

    }
    unmount(parent: HTMLElement): void {
        parent.removeChild(this.videoElement)
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

    private keyboardButton = document.createElement("button")
    private screenKeyboard = new ScreenKeyboard()

    private lockMouseButton = document.createElement("button")

    private fullscreenButton = document.createElement("button")

    private mouseMode: SelectComponent

    private touchMode: SelectComponent

    private config: StreamInputConfig = defaultStreamInputConfig()

    constructor(app: ViewerApp) {
        this.app = app

        // Pop up keyboard
        this.keyboardButton.innerText = "Keyboard"
        this.keyboardButton.addEventListener("click", async event => {
            // This could trigger the screen keyboard listeners for detecting close
            event.stopPropagation()

            setSidebarExtended(false)
            this.screenKeyboard.show()
        })

        this.screenKeyboard.addKeyDownListener(this.onKeyDown.bind(this))
        this.screenKeyboard.addKeyUpListener(this.onKeyUp.bind(this))
        this.screenKeyboard.addTextListener(this.onText.bind(this))

        // Pointer Lock
        this.lockMouseButton.innerText = "Lock Mouse"
        this.lockMouseButton.addEventListener("click", async () => {
            setSidebarExtended(false)

            await app.getElement().requestPointerLock()
        })

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

        // Select Touch Mode
        this.touchMode = new SelectComponent("mouseMode", [
            { value: "touch", name: "Touch" },
            { value: "mouseRelative", name: "Relative" },
            { value: "pointAndDrag", name: "Point and Drag" }
        ], {
            displayName: "Touch Mode",
            preSelectedOption: this.config.touchMode
        })
        this.touchMode.addChangeListener(this.onTouchModeChange.bind(this))
    }

    onCapabilitiesChange(capabilities: StreamCapabilities) {
        this.touchMode.setOptionEnabled("touch", capabilities.touch)
    }

    getScreenKeyboard(): ScreenKeyboard {
        return this.screenKeyboard
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
        parent.appendChild(this.keyboardButton)
        parent.appendChild(this.screenKeyboard.getHiddenElement())
        parent.appendChild(this.lockMouseButton)
        parent.appendChild(this.fullscreenButton)
        this.mouseMode.mount(parent)
        this.touchMode.mount(parent)
    }
    unmount(parent: HTMLElement): void {
        parent.removeChild(this.keyboardButton)
        parent.removeChild(this.screenKeyboard.getHiddenElement())
        parent.removeChild(this.lockMouseButton)
        parent.removeChild(this.fullscreenButton)
        this.mouseMode.unmount(parent)
        this.touchMode.unmount(parent)
    }
}
