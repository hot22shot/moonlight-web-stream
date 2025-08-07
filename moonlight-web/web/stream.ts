import { Api, getApi } from "./api.js";
import { Component } from "./component/index.js";
import { showErrorPopup } from "./component/error.js";
import { App, RtcIceCandidate, RtcSessionDescription, StreamClientMessage, StreamServerMessage } from "./api_bindings.js";
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
    }

    private onAppInfo(event: AppInfoEvent) {
        const app = event.app

        document.title = `Stream: ${app.title}`
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

    constructor() {
        this.test.innerText = "TEST"
    }

    extended(): void {

    }
    unextend(): void {

    }

    mount(parent: HTMLElement): void {
        parent.appendChild(this.test)
    }
    unmount(parent: HTMLElement): void {
        parent.removeChild(this.test)
    }
}

type AppInfoEvent = { app: App } & Event
type AppInfoEventListener = (event: AppInfoEvent) => void

class Stream {
    private api: Api
    private hostId: number
    private appId: number

    private eventTarget = new EventTarget()

    private mediaStream: MediaStream = new MediaStream()

    private ws: WebSocket

    private pc: RTCPeerConnection

    private dataChannel: RTCDataChannel

    constructor(api: Api, hostId: number, appId: number) {
        this.api = api
        this.hostId = hostId
        this.appId = appId

        // Configure web socket
        this.ws = new WebSocket("/api/stream")
        this.ws.onerror = this.onError.bind(this)
        this.ws.onmessage = this.onWsMessage.bind(this);
        this.ws.onopen = this.onWsConnect.bind(this)

        this.sendWsMessage({
            AuthenticateAndInit: {
                credentials: this.api.credentials,
                host_id: this.hostId,
                app_id: this.appId,
            }
        })

        // Configure web rtc
        // Note: We're the polite peer
        this.pc = new RTCPeerConnection({
            iceServers: [{
                urls: ['stun:stun1.l.google.com:19302', 'stun:stun2.l.google.com:19302'],
            }],
            iceCandidatePoolSize: 10,
        })
        this.pc.onnegotiationneeded = this.onNegotiationNeeded.bind(this)
        this.pc.onicecandidate = this.onIceCandidate.bind(this)

        this.pc.ontrack = this.onTrack.bind(this)
        this.pc.ondatachannel = this.onDataChannel.bind(this)
        this.pc.oniceconnectionstatechange = this.onConnectionStateChange.bind(this)

        const videoTransceiver = this.pc.addTransceiver("video", {
            direction: "recvonly",
        })
        videoTransceiver.receiver.jitterBufferTarget = 0

        // TODO: audio
        // const audioTransceiver = this.pc.addTransceiver("audio", {
        //     direction: "recvonly",
        // })
        // audioTransceiver.receiver.jitterBufferTarget = 0

        this.dataChannel = this.pc.createDataChannel("test1")
    }

    private async onNegotiationNeeded() {
        await this.pc.setLocalDescription()
        this.sendLocalDescription()
    }

    private onMessage(message: StreamServerMessage) {
        if (typeof message == "string") {
            (async () => {
                // TODO: make an error event for this
                await showMessage(message)
                window.close()
            })()
        } else if ("UpdateApp" in message) {
            // Dispatch app event
            const app = message.UpdateApp.app

            let event: any = new Event("stream-appinfo")
            event["app"] = app
            this.eventTarget.dispatchEvent(event)
        } else if ("Signaling" in message) {
            if ("Description" in message.Signaling) {
                const descriptionRaw = message.Signaling.Description
                const description = {
                    type: descriptionRaw.ty as RTCSdpType,
                    sdp: descriptionRaw.sdp,
                }

                this.handleSignaling(description, null)

            } else if ("AddIceCandidate" in message.Signaling) {
                const candidateRaw = message.Signaling.AddIceCandidate;
                const candidate: RTCIceCandidateInit = {
                    candidate: candidateRaw.candidate,
                    sdpMid: candidateRaw.sdp_mid,
                    sdpMLineIndex: candidateRaw.sdp_mline_index,
                    usernameFragment: candidateRaw.username_fragment
                }

                this.handleSignaling(null, candidate)
            }
        }
    }
    private async handleSignaling(description: RTCSessionDescriptionInit | null, candidate: RTCIceCandidateInit | null) {
        if (description) {
            await this.setRemoteDescription(description)

            if (description.type == "offer") {
                await this.pc.setLocalDescription()
                this.sendLocalDescription()
            }
        } else if (candidate) {
            this.pc.addIceCandidate(candidate)
        }
    }
    private sendLocalDescription() {
        const description = this.pc.localDescription as RTCSessionDescription;

        this.sendWsMessage({
            Signaling: {
                Description: {
                    ty: description.type,
                    sdp: description.sdp
                }
            }
        })
    }

    private onTrack(event: RTCTrackEvent) {
        console.log(event)
        const stream = event.streams[0]
        if (stream) {
            stream.getTracks().forEach(track => this.mediaStream.addTrack(track))
        }
        // this.mediaStream.addTrack(event.track)
    }
    private onDataChannel(event: RTCDataChannelEvent) {
        console.log(event)

        event.channel.onmessage = msg => {
            console.info(`RECEIVED: ${event.channel.label}, ${msg.data}`)
            this.dataChannel.send(msg.data)
        }

    }
    private onIceCandidate(event: RTCPeerConnectionIceEvent) {
        const candidateJson = event.candidate?.toJSON()
        if (!candidateJson || !candidateJson?.candidate) {
            return;
        }

        const candidate: RtcIceCandidate = {
            candidate: candidateJson?.candidate,
            sdp_mid: candidateJson?.sdpMid ?? null,
            sdp_mline_index: candidateJson?.sdpMLineIndex ?? null,
            username_fragment: candidateJson?.usernameFragment ?? null
        }

        this.sendWsMessage({
            Signaling: {
                AddIceCandidate: candidate
            }
        })
    }
    private onConnectionStateChange(event: Event) {
        console.log(this.pc.connectionState)
    }

    // -- Raw Web Socket stuff
    private wsMessageBuffer: Array<StreamClientMessage> = []
    private sendWsMessage(message: StreamClientMessage) {
        if (this.ws.readyState == WebSocket.OPEN) {
            this.ws.send(JSON.stringify(message))
        } else {
            this.wsMessageBuffer.push(message)
        }
    }
    private async onWsConnect() {
        for (const message of this.wsMessageBuffer.splice(0, this.wsMessageBuffer.length)) {
            this.ws.send(JSON.stringify(message))
        }
    }

    private onWsMessage(event: MessageEvent) {
        const data = event.data
        if (typeof data != "string") {
            return
        }

        let message = JSON.parse(data)
        this.onMessage(message)
    }

    private onError(event: Event) {
        console.error("Stream Error", event)
    }

    // -- Raw Peer Connection stuff
    private iceCandidateBuffer: Array<RTCIceCandidateInit> = []
    private addIceCandidate(candidate: RTCIceCandidateInit) {
        if (this.pc.remoteDescription != null) {
            this.pc.addIceCandidate(candidate)
        } else {
            this.iceCandidateBuffer.push(candidate)
        }
    }

    private async setRemoteDescription(description: RTCSessionDescriptionInit) {
        await this.pc.setRemoteDescription(description)

        // add buffered ice candidates
        for (const candidate of this.iceCandidateBuffer.splice(0, this.iceCandidateBuffer.length)) {
            this.addIceCandidate(candidate)
        }
    }

    // -- Class Api
    addAppInfoListener(listener: AppInfoEventListener) {
        this.eventTarget.addEventListener("stream-appinfo", listener as EventListenerOrEventListenerObject)
    }
    removeAppInfoListener(listener: AppInfoEventListener) {
        this.eventTarget.removeEventListener("stream-appinfo", listener as EventListenerOrEventListenerObject)
    }

    getMediaStream(): MediaStream {
        return this.mediaStream
    }
}
