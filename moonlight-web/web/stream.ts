import { Api, getApi } from "./api.js";
import { Component } from "./component/index.js";
import { showErrorPopup } from "./component/error.js";
import { AppInfoEvent, Stream } from "./stream/index.js"
import { showMessage } from "./component/modal/index.js";
import { setSidebar, Sidebar } from "./component/sidebar/index.js";

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

    private stream: Stream

    constructor(api: Api, hostId: number, appId: number) {
        this.api = api

        // Configure sidebar
        this.sidebar = new ViewerSidebar()
        setSidebar(this.sidebar)

        // Configure stream
        this.stream = new Stream(api, hostId, appId)
        this.stream.addAppInfoListener(this.onAppInfo.bind(this))

        // Configure video element
        this.videoElement.classList.add("video-stream")
        this.videoElement.controls = false
        this.videoElement.autoplay = true
        this.videoElement.srcObject = this.stream.getMediaStream()

        // Configure input
        document.addEventListener("keydown", this.onKeyDown.bind(this))
        document.addEventListener("keyup", this.onKeyUp.bind(this))
    }

    private onAppInfo(event: AppInfoEvent) {
        const app = event.app

        document.title = `Stream: ${app.title}`
    }

    private onKeyDown(event: KeyboardEvent) {
        console.log("DOWN", event)
        this.stream.getInput().getKeyboard().onKeyDown(event)
    }
    private onKeyUp(event: KeyboardEvent) {
        console.log("UP", event)
        this.stream.getInput().getKeyboard().onKeyUp(event)
    }

    mount(parent: HTMLElement): void {
        parent.appendChild(this.videoElement)

    }
    unmount(parent: HTMLElement): void {
        parent.removeChild(this.videoElement)
    }
}

class ViewerSidebar implements Component, Sidebar {

    private test: HTMLElement = document.createElement("p")
    private keyboardButton = document.createElement("button")
    private keyboardHiddenInput = document.createElement("textarea")

    constructor() {
        this.test.innerText = "TEST"

        this.keyboardButton.innerText = "Keyboard"
        this.keyboardButton.addEventListener("click", async () => {
            this.keyboardHiddenInput.focus()
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
    }
    unmount(parent: HTMLElement): void {
        parent.removeChild(this.test)
        parent.removeChild(this.keyboardButton)
        parent.removeChild(this.keyboardHiddenInput)
    }
}
