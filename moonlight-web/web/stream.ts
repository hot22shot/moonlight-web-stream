import { Api, getApi } from "./api.js";
import { Component } from "./component/index.js";
import { showErrorPopup } from "./component/error.js";
import { AppInfoEvent, startStream, Stream } from "./stream/index.js"
import { showMessage } from "./component/modal/index.js";
import { setSidebar, setSidebarExtended, Sidebar } from "./component/sidebar/index.js";
import { StreamKeys } from "./api_bindings.js";

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

        this.videoElement.addEventListener("touchstart", this.onTouchStart.bind(this))
        this.videoElement.addEventListener("touchend", this.onTouchEnd.bind(this))
        this.videoElement.addEventListener("touchmove", this.onTouchMove.bind(this))
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
        this.stream?.getInput().getKeyboard().onKeyDown(event)
    }
    onKeyUp(event: KeyboardEvent) {
        event.preventDefault()
        this.stream?.getInput().getKeyboard().onKeyUp(event)
    }

    // Mouse
    onMouseButtonDown(event: MouseEvent) {
        event.preventDefault()
        this.stream?.getInput().getMouse().onMouseDown(event);
    }
    onMouseButtonUp(event: MouseEvent) {
        event.preventDefault()
        this.stream?.getInput().getMouse().onMouseUp(event)
    }
    onMouseMove(event: MouseEvent) {
        event.preventDefault()
        this.stream?.getInput().getMouse().onMouseMove(event)
    }
    onWheel(event: WheelEvent) {
        event.preventDefault()
        this.stream?.getInput().getMouse().onWheel(event)
    }

    // Touch
    onTouchStart(event: TouchEvent) {
        this.stream?.getInput().getTouch().onTouchStart(event, this.videoElement.getBoundingClientRect())
    }
    onTouchEnd(event: TouchEvent) {
        this.stream?.getInput().getTouch().onTouchEnd(event, this.videoElement.getBoundingClientRect())
    }
    onTouchMove(event: TouchEvent) {
        this.stream?.getInput().getTouch().onTouchMove(event, this.videoElement.getBoundingClientRect())
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

    private test: HTMLElement = document.createElement("p")
    private keyboardButton = document.createElement("button")
    private keyboardInput = document.createElement("input")

    private lockMouseButton = document.createElement("button")

    constructor(app: ViewerApp) {
        this.app = app

        this.test.innerText = "TEST"

        // Pop up keyboard
        // TODO: try to push stream up to account for the pop up keyboard
        this.keyboardButton.innerText = "Keyboard"
        this.keyboardButton.addEventListener("click", async () => {
            setSidebarExtended(false)

            this.keyboardInput.focus()
        })

        this.keyboardInput.classList.add("hiddeninput")
        this.keyboardInput.autocomplete = "off"
        this.keyboardInput.autocapitalize = "off"
        this.keyboardInput.spellcheck = false
        if ("autocorrect" in this.keyboardInput) {
            this.keyboardInput.autocorrect = false
        }
        this.keyboardInput.addEventListener("input", this.onInput.bind(this))
        this.keyboardInput.addEventListener("keydown", this.onKeyDown.bind(this))
        this.keyboardInput.addEventListener("keyup", this.onKeyUp.bind(this))

        // Pointer Lock
        this.lockMouseButton.innerText = "Lock Mouse"
        this.lockMouseButton.addEventListener("click", async () => {
            setSidebarExtended(false)

            await app.getElement().requestPointerLock()
        })
    }

    private onInput(event: Event) {
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
        const keyboard = stream.getInput().getKeyboard()

        if ((event.inputType == "insertText" || event.inputType == "insertFromPaste") && event.data != null) {
            keyboard.sendText(event.data)
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
        const keyboard = stream.getInput().getKeyboard()

        keyboard.onKeyDown(event)
    }
    private onKeyUp(event: KeyboardEvent) {
        const stream = this.app.getStream()
        if (!stream) {
            return
        }
        const keyboard = stream.getInput().getKeyboard()

        keyboard.onKeyUp(event)
    }

    extended(): void {

    }
    unextend(): void {

    }

    mount(parent: HTMLElement): void {
        parent.appendChild(this.test)
        parent.appendChild(this.keyboardButton)
        parent.appendChild(this.keyboardInput)
        parent.appendChild(this.lockMouseButton)
    }
    unmount(parent: HTMLElement): void {
        parent.removeChild(this.test)
        parent.removeChild(this.keyboardButton)
        parent.removeChild(this.keyboardInput)
        parent.removeChild(this.lockMouseButton)
    }
}
