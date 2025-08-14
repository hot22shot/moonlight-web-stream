import { Api } from "../api.js"
import { App, RtcIceCandidate, StreamClientMessage, StreamServerMessage } from "../api_bindings.js"
import { showMessage } from "../component/modal/index.js"
import { StreamSettings } from "../component/settings_menu.js"
import { defaultStreamInputConfig, StreamInput } from "./input.js"

export function startStream(api: Api, hostId: number, appId: number, settings: StreamSettings, viewerScreenSize: [number, number]): Promise<Stream> {
    return new Promise((resolve, reject) => {
        const ws = new WebSocket(`${api.host_url}/stream`)
        ws.onopen = () => {
            const stream = new Stream(ws, api, hostId, appId, settings, viewerScreenSize)

            resolve(stream)
        };
        ws.onerror = (error) => {
            reject(error);
        };
    })
}

export type AppInfoEvent = { app: App } & Event
export type AppInfoEventListener = (event: AppInfoEvent) => void

export class Stream {
    private api: Api
    private hostId: number
    private appId: number

    private settings: StreamSettings

    private eventTarget = new EventTarget()

    private mediaStream: MediaStream = new MediaStream()
    private input: StreamInput

    private ws: WebSocket

    private peer: RTCPeerConnection

    constructor(ws: WebSocket, api: Api, hostId: number, appId: number, settings: StreamSettings, viewerScreenSize: [number, number]) {
        if (ws.readyState != WebSocket.OPEN) {
            throw "WebSocket is not open whilst starting stream"
        }

        this.api = api
        this.hostId = hostId
        this.appId = appId

        this.settings = settings

        // Configure web socket
        this.ws = ws
        this.ws.onerror = this.onError.bind(this)
        this.ws.onmessage = this.onWsMessage.bind(this);

        this.sendWsMessage({
            AuthenticateAndInit: {
                credentials: this.api.credentials,
                host_id: this.hostId,
                app_id: this.appId,
                bitrate: this.settings.bitrate,
                packet_size: this.settings.packetSize,
                fps: this.settings.fps,
                width: this.settings.videoSize?.width ?? viewerScreenSize[0],
                height: this.settings.videoSize?.height ?? viewerScreenSize[1],
                video_sample_queue_size: this.settings.videoSampleQueueSize
            }
        })

        // Configure web rtc
        this.peer = new RTCPeerConnection({
            // iceServers: [
            //     { urls: "stun:stun.l.google.com:19302" },
            //     { urls: "stun:stun.l.google.com:5349" },
            //     { urls: "stun:stun1.l.google.com:3478" },
            //     { urls: "stun:stun1.l.google.com:5349" },
            //     { urls: "stun:stun2.l.google.com:19302" },
            //     { urls: "stun:stun2.l.google.com:5349" },
            //     { urls: "stun:stun3.l.google.com:3478" },
            //     { urls: "stun:stun3.l.google.com:5349" },
            //     { urls: "stun:stun4.l.google.com:19302" },
            //     { urls: "stun:stun4.l.google.com:5349" }
            // ],
        })
        this.peer.onnegotiationneeded = this.onNegotiationNeeded.bind(this)
        this.peer.onicecandidate = this.onIceCandidate.bind(this)

        this.peer.ontrack = this.onTrack.bind(this)
        this.peer.ondatachannel = this.onDataChannel.bind(this)
        this.peer.oniceconnectionstatechange = this.onConnectionStateChange.bind(this)

        const videoTransceiver = this.peer.addTransceiver("video", {
            direction: "recvonly",
        })
        videoTransceiver.receiver.jitterBufferTarget = 0

        // TODO: audio
        // const audioTransceiver = this.pc.addTransceiver("audio", {
        //     direction: "recvonly",
        // })
        // audioTransceiver.receiver.jitterBufferTarget = 0

        const streamInputConfig = defaultStreamInputConfig()
        Object.assign(streamInputConfig, {
            controllerConfig: settings.controllerConfig
        })
        this.input = new StreamInput(this.peer, streamInputConfig)
    }

    private async onMessage(message: StreamServerMessage) {
        if (typeof message == "string") {
            // TODO: make an error event for this
            await showMessage(message)

            if (message == "PeerDisconnect") {
                location.reload()
            } else {
                window.close()
            }
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

                await this.handleSignaling(description, null)
            } else if ("AddIceCandidate" in message.Signaling) {
                const candidateRaw = message.Signaling.AddIceCandidate;
                const candidate: RTCIceCandidateInit = {
                    candidate: candidateRaw.candidate,
                    sdpMid: candidateRaw.sdp_mid,
                    sdpMLineIndex: candidateRaw.sdp_mline_index,
                    usernameFragment: candidateRaw.username_fragment
                }

                await this.handleSignaling(null, candidate)
            }
        }
    }

    // -- Signaling
    private async onNegotiationNeeded() {
        const offer = await this.peer.createOffer()
        await this.peer.setLocalDescription(offer)

        await this.sendLocalDescription()
    }
    private async handleSignaling(description: RTCSessionDescriptionInit | null, candidate: RTCIceCandidateInit | null) {
        if (description) {
            await this.peer.setRemoteDescription(description)

            if (description.type === "offer") {
                const answer = await this.peer.createAnswer()
                await this.peer.setLocalDescription(answer)

                await this.sendLocalDescription()
            }
        } else if (candidate) {
            this.peer.addIceCandidate(candidate)
        }
    }
    private sendLocalDescription() {
        const description = this.peer.localDescription as RTCSessionDescription;

        this.sendWsMessage({
            Signaling: {
                Description: {
                    ty: description.type,
                    sdp: description.sdp
                }
            }
        })
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

    // -- Track and Data Channels
    private onTrack(event: RTCTrackEvent) {
        console.log(event)
        const stream = event.streams[0]
        if (stream) {
            stream.getTracks().forEach(track => this.mediaStream.addTrack(track))
        }
        // this.mediaStream.addTrack(event.track)
    }
    private onDataChannel(event: RTCDataChannelEvent) {
        // TODO: remove
        console.log(event)

        event.channel.onmessage = msg => console.log(`Msg from ${event.channel.label}`, msg.data)
    }
    private onConnectionStateChange(event: Event) {
        console.log(this.peer.connectionState)
    }

    // -- Raw Web Socket stuff
    private sendWsMessage(message: StreamClientMessage) {
        this.ws.send(JSON.stringify(message))
    }

    private async onWsMessage(event: MessageEvent) {
        const data = event.data
        if (typeof data != "string") {
            return
        }

        let message = JSON.parse(data)
        await this.onMessage(message)
    }

    private onError(event: Event) {
        console.error("Stream Error", event)
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

    getInput(): StreamInput {
        return this.input
    }
}