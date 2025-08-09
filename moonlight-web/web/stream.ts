import { Api, getApi } from "./api.js";
import { Component } from "./component/index.js";
import { showErrorPopup } from "./component/error.js";
import { AppInfoEvent, startStream, Stream } from "./stream/index.js"
import { showMessage } from "./component/modal/index.js";
import { setSidebar, setSidebarExtended, Sidebar } from "./component/sidebar/index.js";

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

        document.addEventListener("mousedown", this.onMouseButtonDown.bind(this))
        document.addEventListener("mouseup", this.onMouseButtonUp.bind(this))
        document.addEventListener("mousemove", this.onMouseMove.bind(this))
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
    private onKeyDown(event: KeyboardEvent) {
        event.preventDefault()
        this.stream?.getInput().getKeyboard().onKeyDown(event)
    }
    private onKeyUp(event: KeyboardEvent) {
        event.preventDefault()
        this.stream?.getInput().getKeyboard().onKeyUp(event)
    }

    // Mouse
    private onMouseButtonDown(event: MouseEvent) {
        event.preventDefault()
        this.stream?.getInput().getMouse().onMouseDown(event);
    }
    private onMouseButtonUp(event: MouseEvent) {
        event.preventDefault()
        this.stream?.getInput().getMouse().onMouseUp(event)
    }
    private onMouseMove(event: MouseEvent) {
        event.preventDefault()
        this.stream?.getInput().getMouse().onMouseMove(event)
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
}

class ViewerSidebar implements Component, Sidebar {
    private app: ViewerApp

    private test: HTMLElement = document.createElement("p")
    private keyboardButton = document.createElement("button")
    private keyboardHiddenInput = document.createElement("textarea")

    private lockMouseButton = document.createElement("button")

    constructor(app: ViewerApp) {
        this.app = app

        this.test.innerText = "TEST"

        this.keyboardButton.innerText = "Keyboard"
        this.keyboardButton.addEventListener("click", async () => {
            setSidebarExtended(false)

            this.keyboardHiddenInput.focus()
        })

        this.lockMouseButton.innerText = "Lock Mouse"
        this.lockMouseButton.addEventListener("click", async () => {
            setSidebarExtended(false)

            await app.getElement().requestPointerLock()
        })
    }

    extended(): void {

    }
    unextend(): void {

    }

    mount(parent: HTMLElement): void {
        parent.appendChild(this.test)
        parent.appendChild(this.keyboardButton)
        parent.appendChild(this.keyboardHiddenInput)
        parent.appendChild(this.lockMouseButton)
    }
    unmount(parent: HTMLElement): void {
        parent.removeChild(this.test)
        parent.removeChild(this.keyboardButton)
        parent.removeChild(this.keyboardHiddenInput)
        parent.removeChild(this.lockMouseButton)
    }
}
