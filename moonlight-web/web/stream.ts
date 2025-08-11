import { Api, getApi } from "./api.js";
import { Component } from "./component/index.js";
import { showErrorPopup } from "./component/error.js";
import { AppInfoEvent, startStream, Stream } from "./stream/index.js"
import { showMessage } from "./component/modal/index.js";
import { setSidebar, setSidebarExtended, Sidebar } from "./component/sidebar/index.js";
import { defaultStreamInputConfig, StreamInputConfig } from "./stream/input.js";

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
        this.videoElement.controls = false
        this.videoElement.autoplay = true

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
    }

    private async startStream(hostId: number, appId: number) {
        this.stream = await startStream(this.api, hostId, appId)

        // Add app info listener
        this.stream.addAppInfoListener(this.onAppInfo.bind(this))

        // Set video
        this.videoElement.srcObject = this.stream.getMediaStream()
    }

    private onAppInfo(event: AppInfoEvent) {
        const app = event.app

        document.title = `Stream: ${app.title}`
    }

    // Keyboard
    onKeyDown(event: KeyboardEvent) {
        event.preventDefault()
        this.stream?.getInput().onKeyDown(event)
    }
    onKeyUp(event: KeyboardEvent) {
        event.preventDefault()
        this.stream?.getInput().onKeyUp(event)
    }

    // Mouse
    onMouseButtonDown(event: MouseEvent) {
        event.preventDefault()
        this.stream?.getInput().onMouseDown(event, this.videoElement.getBoundingClientRect());
    }
    onMouseButtonUp(event: MouseEvent) {
        event.preventDefault()
        this.stream?.getInput().onMouseUp(event)
    }
    onMouseMove(event: MouseEvent) {
        event.preventDefault()
        this.stream?.getInput().onMouseMove(event)
    }
    onWheel(event: WheelEvent) {
        event.preventDefault()
        this.stream?.getInput().onWheel(event)
    }

    // Touch
    onTouchStart(event: TouchEvent) {
        event.preventDefault()
        this.stream?.getInput().onTouchStart(event, this.videoElement.getBoundingClientRect())
    }
    onTouchEnd(event: TouchEvent) {
        event.preventDefault()
        this.stream?.getInput().onTouchEnd(event, this.videoElement.getBoundingClientRect())
    }
    onTouchMove(event: TouchEvent) {
        event.preventDefault()
        this.stream?.getInput().onTouchMove(event, this.videoElement.getBoundingClientRect())
    }

    // Gamepad
    onGamepadConnect(event: GamepadEvent) {
        console.log(event)
        this.stream?.getInput().onGamepadConnect(event)
    }
    onGamepadDisconnect(event: GamepadEvent) {
        console.log(event)
        this.stream?.getInput().onGamepadDisconnect(event)
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

    private mouseModeDiv = document.createElement("div")
    private mouseModeLabel = document.createElement("label")
    private mouseModeSelect = document.createElement("select")

    private touchModeDiv = document.createElement("div")
    private touchModeLabel = document.createElement("label")
    private touchModeSelect = document.createElement("select")

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

        // Select Mouse Mode
        this.mouseModeLabel.htmlFor = "mouseMode"
        this.mouseModeLabel.innerText = "Mouse Mode"

        this.mouseModeDiv.appendChild(this.mouseModeLabel)

        this.mouseModeSelect.name = "mouseMode"
        this.mouseModeSelect.addEventListener("change", this.onMouseModeChange.bind(this))

        const mouseModeRelative = document.createElement("option")
        mouseModeRelative.value = "relative"
        mouseModeRelative.innerText = "Relative"
        this.mouseModeSelect.appendChild(mouseModeRelative)

        const mouseModePointAndDrag = document.createElement("option")
        mouseModePointAndDrag.value = "pointAndDrag"
        mouseModePointAndDrag.innerText = "Point and Drag"
        this.mouseModeSelect.appendChild(mouseModePointAndDrag)

        if (this.config.mouseMode == "relative") {
            this.mouseModeSelect.selectedIndex = 0
        } else if (this.config.mouseMode == "pointAndDrag") {
            this.mouseModeSelect.selectedIndex = 1
        } else {
            throw ""
        }

        this.mouseModeDiv.appendChild(this.mouseModeSelect)

        // Select Touch Mode
        this.touchModeLabel.htmlFor = "touchMode"
        this.touchModeLabel.innerText = "Touch Mode"

        this.touchModeDiv.appendChild(this.touchModeLabel)

        this.touchModeSelect.name = "touchMode"
        this.touchModeSelect.value = this.config.touchMode
        this.touchModeSelect.addEventListener("change", this.onTouchModeChange.bind(this))

        const touchModeTouch = document.createElement("option")
        touchModeTouch.value = "touch"
        touchModeTouch.innerText = "Touch"
        this.touchModeSelect.appendChild(touchModeTouch)

        const touchModeRelative = document.createElement("option")
        touchModeRelative.value = "mouseRelative"
        touchModeRelative.innerText = "Relative"
        this.touchModeSelect.appendChild(touchModeRelative)

        const touchModePointAndDrag = document.createElement("option")
        touchModePointAndDrag.value = "pointAndDrag"
        touchModePointAndDrag.innerText = "Point and Drag"
        this.touchModeSelect.appendChild(touchModePointAndDrag)

        if (this.config.touchMode == "touch") {
            this.touchModeSelect.selectedIndex = 0
        } else if (this.config.touchMode == "mouseRelative") {
            this.touchModeSelect.selectedIndex = 1
        } else if (this.config.touchMode == "pointAndDrag") {
            this.touchModeSelect.selectedIndex = 2
        } else {
            throw ""
        }

        this.touchModeDiv.appendChild(this.touchModeSelect)
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
    private onMouseModeChange(event: Event) {
        this.config.mouseMode = this.mouseModeSelect.value as "relative" | "pointAndDrag"
        this.app.getStream()?.getInput().setConfig(this.config)
    }

    // -- Touch Mode
    private onTouchModeChange(event: Event) {
        this.config.touchMode = this.touchModeSelect.value as "touch" | "mouseRelative" | "pointAndDrag"
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
        parent.appendChild(this.mouseModeDiv)
        parent.appendChild(this.touchModeDiv)
    }
    unmount(parent: HTMLElement): void {
        parent.removeChild(this.keyboardButton)
        parent.removeChild(this.keyboardInput)
        parent.removeChild(this.lockMouseButton)
        parent.removeChild(this.mouseModeDiv)
        parent.removeChild(this.touchModeDiv)
    }
}
