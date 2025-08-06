import { Api, getApi } from "./api.js";
import { Component, ComponentHost } from "./component/index.js";
import { showErrorPopup } from "./component/error.js";
import { App, RtcIceCandidate, StreamClientMessage, StreamServerMessage } from "./api_bindings.js";
import { showMessage } from "./component/modal/index.js";

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
    const root = new ComponentHost(rootElement, app)
}

startApp()

class ViewerApp implements Component {
    private api: Api

    private videoElement = document.createElement("video")

    private stream: Stream

    constructor(api: Api, hostId: number, appId: number) {
        this.api = api

        // Configure stream
        this.stream = new Stream(api, hostId, appId)
        this.stream.addAppInfoListener(this.onAppInfo.bind(this))

        // Configure video element
        this.videoElement.controls = false
        this.videoElement.autoplay = true
        this.videoElement.playsInline = true
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

type AppInfoEvent = { app: App } & Event
type AppInfoEventListener = (event: AppInfoEvent) => void

class Stream {
    private api: Api
    private hostId: number
    private appId: number

    private eventTarget = new EventTarget()

    private mediaStream: MediaStream = new MediaStream()

    private ws: WebSocket
    private rtc: RTCPeerConnection

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

        // Configure web rtc
        this.rtc = new RTCPeerConnection({
            iceServers: [{
                urls: ['stun:stun1.l.google.com:19302', 'stun:stun2.l.google.com:19302'],
            }],
            iceCandidatePoolSize: 10,
        })
        this.rtc.onicecandidate = this.onIceCandidate.bind(this)

        this.rtc.ontrack = this.onTrack.bind(this)
        this.rtc.ondatachannel = this.onDataChannel.bind(this)
        this.rtc.oniceconnectionstatechange = this.onConnectionStateChange.bind(this)

        const transceiver = this.rtc.addTransceiver("video", {
            direction: "recvonly",
        })
        transceiver.receiver.jitterBufferTarget = 0

        // this.rtc.addTransceiver("audio", {
        //     direction: "recvonly",
        // })
        this.dataChannel = this.rtc.createDataChannel("test1")
    }

    // Send Offer
    private async onWsConnect() {
        const offer = await this.rtc.createOffer()
        if (!offer || !offer.sdp) {
            throw "failed to create offer"
        }

        await this.rtc.setLocalDescription(offer)

        console.info("sending offer")
        this.sendWsMessage({
            Offer: {
                credentials: this.api.credentials,
                offer_sdp: offer.sdp,
                host_id: this.hostId,
                app_id: this.appId,
            }
        })
    }

    private onMessage(message: StreamServerMessage) {
        if (typeof message == "string") {
            (async () => {
                // TODO: make an error event for this
                await showMessage(message)
                window.close()
            })()
        } else if ("AddIceCandidate" in message) {
            const candidate = message.AddIceCandidate.candidate;
            const candidateJson: RTCIceCandidateInit = {
                candidate: candidate.candidate,
                sdpMid: candidate.sdp_mid,
                sdpMLineIndex: candidate.sdp_mline_index,
                usernameFragment: candidate.username_fragment
            }

            this.addIceCandidate(candidateJson)
        } else if ("Answer" in message) {
            console.info("received answer")

            // Set Remote Description
            const answer_sdp = message.Answer.answer_sdp

            this.setRemoteDescription({
                type: "answer",
                sdp: answer_sdp
            })

            // Dispatch app event
            const app = message.Answer.app

            let event: any = new Event("stream-appinfo")
            event["app"] = app
            this.eventTarget.dispatchEvent(event)
        }
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
            AddIceCandidate: {
                candidate
            }
        })
    }
    private onConnectionStateChange(event: Event) {
        console.log(this.rtc.connectionState)
    }

    // -- Raw Web Socket stuff
    private wsMessageBuffer: Array<StreamClientMessage> = []
    private sendWsMessage(message: StreamClientMessage) {
        if (this.ws.readyState == WebSocket.OPEN) {
            this.ws.send(JSON.stringify(message))

            if (this.wsMessageBuffer.length != 0) {
                for (const message of this.wsMessageBuffer.splice(0, this.wsMessageBuffer.length)) {
                    this.ws.send(JSON.stringify(message))
                }
            }
        } else {
            this.wsMessageBuffer.push(message)
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
        if (this.rtc.remoteDescription != null) {
            this.rtc.addIceCandidate(candidate)
        } else {
            this.iceCandidateBuffer.push(candidate)
        }
    }

    private async setRemoteDescription(description: RTCSessionDescriptionInit) {
        await this.rtc.setRemoteDescription(description)

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
