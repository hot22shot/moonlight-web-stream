import { Api, getApi } from "./api.js";
import { Component } from "./component/index.js";
import { showErrorPopup } from "./component/error.js";
import { AppInfoEvent, startStream, Stream } from "./stream/index.js"
import { showMessage } from "./component/modal/index.js";
import { setSidebar, setSidebarExtended, Sidebar } from "./component/sidebar/index.js";
import { defaultStreamInputConfig, StreamInputConfig } from "./stream/input.js";
import { defaultStreamSettings, getLocalStreamSettings } from "./component/settings_menu.js";
import { SelectComponent } from "./component/input.js";

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
        document.addEventListener("keydown", this.onKeyDown.bind(this))
        document.addEventListener("keyup", this.onKeyUp.bind(this))

        this.videoElement.addEventListener("mousedown", this.onMouseButtonDown.bind(this))
        this.videoElement.addEventListener("mouseup", this.onMouseButtonUp.bind(this))
        this.videoElement.addEventListener("mousemove", this.onMouseMove.bind(this))
        this.videoElement.addEventListener("wheel", this.onWheel.bind(this))

        this.videoElement.addEventListener("touchstart", this.onTouchStart.bind(this), { passive: false })
        this.videoElement.addEventListener("touchend", this.onTouchEnd.bind(this), { passive: false })
        this.videoElement.addEventListener("touchmove", this.onTouchMove.bind(this), { passive: false })

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

        this.stream = await startStream(this.api, hostId, appId, getLocalStreamSettings() ?? defaultStreamSettings(), [viewerWidth, viewerHeight])

        // Add app info listener
        this.stream.addAppInfoListener(this.onAppInfo.bind(this))

        // Set video
        this.videoElement.srcObject = this.stream.getMediaStream()

        // Start animation frame loop
        this.onGamepadUpdate()
    }

    private onAppInfo(event: AppInfoEvent) {
        const app = event.app

        document.title = `Stream: ${app.title}`
    }

    onUserInteraction() {
        this.videoElement.muted = false
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
        this.onUserInteraction()

        event.preventDefault()
        this.stream?.getInput().onMouseMove(event, this.videoElement.getBoundingClientRect())
    }
    onWheel(event: WheelEvent) {
        this.onUserInteraction()

        event.preventDefault()
        this.stream?.getInput().onWheel(event)
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
    onTouchMove(event: TouchEvent) {
        this.onUserInteraction()

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
        try {
            this.stream?.getInput().onGamepadUpdate()
        } catch (e: any) {
            alert(e.toString())
        }

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

class ViewerSidebar implements Component, Sidebar {
    private app: ViewerApp

    private keyboardButton = document.createElement("button")
    private keyboardInput = document.createElement("input")

    private lockMouseButton = document.createElement("button")

    private fullscreenButton = document.createElement("button")

    private mouseMode: SelectComponent

    private touchMode: SelectComponent

    private config: StreamInputConfig = defaultStreamInputConfig()

    constructor(app: ViewerApp) {
        this.app = app

        // Pop up keyboard
        // TODO: try to push stream up to account for the pop up keyboard
        this.keyboardButton.innerText = "Keyboard"
        document.addEventListener("click", event => {
            if (event.target != this.keyboardButton) {
                this.keyboardInput.blur()
            }
        })
        this.keyboardButton.addEventListener("click", async () => {
            setSidebarExtended(false)

            this.keyboardInput.focus()
        })

        this.keyboardInput.classList.add("hiddeninput")
        this.keyboardInput.name = "keyboard"
        this.keyboardInput.autocomplete = "off"
        this.keyboardInput.autocapitalize = "off"
        this.keyboardInput.spellcheck = false
        if ("autocorrect" in this.keyboardInput) {
            this.keyboardInput.autocorrect = false
        }
        this.keyboardInput.addEventListener("input", this.onKeyInput.bind(this))
        this.keyboardInput.addEventListener("keydown", this.onKeyDown.bind(this))
        this.keyboardInput.addEventListener("keyup", this.onKeyUp.bind(this))

        // Pointer Lock
        this.lockMouseButton.innerText = "Lock Mouse"
        this.lockMouseButton.addEventListener("click", async () => {
            setSidebarExtended(false)

            await app.getElement().requestPointerLock()
        })

        // Fullscreen
        this.fullscreenButton.innerText = "Fullscreen"
        this.fullscreenButton.addEventListener("click", () => {
            const root = document.getElementById("root")
            if (root) {
                root.requestFullscreen({
                    navigationUI: "hide"
                })
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

    // -- Keyboard
    private onKeyInput(event: Event) {
        if (!(event instanceof InputEvent)) {
            return
        }
        if (event.isComposing) {
            return
        }

        const stream = this.app.getStream()
        if (!stream) {
            return
        }

        if ((event.inputType == "insertText" || event.inputType == "insertFromPaste") && event.data != null) {
            stream.getInput().sendText(event.data)
        } else if (event.inputType == "deleteContentBackward" || event.inputType == "deleteByCut") {
            // these are handled by on key down / up on mobile
        } else if (event.inputType == "deleteContentForward") {
            // these are handled by on key down / up on mobile
        }
    }
    private onKeyDown(event: KeyboardEvent) {
        const stream = this.app.getStream()
        if (!stream) {
            return
        }

        stream.getInput().onKeyDown(event)
    }
    private onKeyUp(event: KeyboardEvent) {
        const stream = this.app.getStream()
        if (!stream) {
            return
        }

        stream.getInput().onKeyUp(event)
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
        parent.appendChild(this.keyboardInput)
        parent.appendChild(this.lockMouseButton)
        parent.appendChild(this.fullscreenButton)
        this.mouseMode.mount(parent)
        this.touchMode.mount(parent)
    }
    unmount(parent: HTMLElement): void {
        parent.removeChild(this.keyboardButton)
        parent.removeChild(this.keyboardInput)
        parent.removeChild(this.lockMouseButton)
        parent.removeChild(this.fullscreenButton)
        this.mouseMode.unmount(parent)
        this.touchMode.unmount(parent)
    }
}
