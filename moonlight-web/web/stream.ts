import { Api, getApi } from "./api";
import { Component, ComponentHost } from "./component";
import { showErrorPopup } from "./component/error";

async function startApp() {
    const api = await getApi()

    const rootElement = document.getElementById("root");
    if (rootElement == null) {
        showErrorPopup("couldn't find root element", true)
        return;
    }

    // Get Host and App via Query
    const queryParams = new URLSearchParams(location.href)

    const hostIdStr = queryParams.get("")
    const appIdStr = queryParams.get("")
    if (hostIdStr == null || appIdStr == null) {
        return
    }
    const hostId = Number.parseInt(hostIdStr)
    const appId = Number.parseInt(appIdStr)

    // Start and Mount App
    const app = new ViewerApp(api, hostId, appId)
    const root = new ComponentHost(rootElement, app)
}

startApp()

class ViewerApp implements Component {
    private api: Api

    private hostId: number
    private appId: number

    private videoElement = document.createElement("video")

    private rtc: RTCPeerConnection = new RTCPeerConnection()
    private mediaStream: MediaStream = new MediaStream()

    constructor(api: Api, hostId: number, appId: number) {
        this.api = api

        this.hostId = hostId
        this.appId = appId

        // Video Element
        this.videoElement.srcObject = this.mediaStream

        // Configure rtc
        this.rtc.ontrack = event => {
            this.mediaStream.addTrack(event.track)
        }

        this.connectRtc()
    }

    private async connectRtc() { }

    mount(parent: HTMLElement): void {

    }
    unmount(parent: HTMLElement): void {

    }
}