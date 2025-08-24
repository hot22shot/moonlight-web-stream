import { Api } from "../api.js"
import { App, ConnectionStatus, RtcIceCandidate, StreamCapabilities, StreamClientMessage, StreamServerGeneralMessage, StreamServerMessage } from "../api_bindings.js"
import { StreamSettings } from "../component/settings_menu.js"
import { defaultStreamInputConfig, StreamInput } from "./input.js"
import { createSupportedVideoFormatsBits, VideoCodecSupport } from "./video.js"

export type InfoEvent = CustomEvent<
    { type: "app", app: App } |
    { type: "error", message: string } |
    { type: "stageStarting" | "stageComplete", stage: string } |
    { type: "stageFailed", stage: string, errorCode: number } |
    { type: "connectionComplete", capabilities: StreamCapabilities } |
    { type: "connectionStatus", status: ConnectionStatus } |
    { type: "connectionTerminated", errorCode: number }
>
export type InfoEventListener = (event: InfoEvent) => void

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

    constructor(api: Api, hostId: number, appId: number, settings: StreamSettings, supported_video_formats: VideoCodecSupport, viewerScreenSize: [number, number]) {
        this.api = api
        this.hostId = hostId
        this.appId = appId

        this.settings = settings

        // Configure web socket
        this.ws = new WebSocket(`${api.host_url}/host/stream`)
        this.ws.addEventListener("error", this.onError.bind(this))
        this.ws.addEventListener("open", this.onWsOpen.bind(this))
        this.ws.addEventListener("message", this.onRawWsMessage.bind(this))

        let width, height
        if (this.settings.videoSize == "720p") {
            width = 1280
            height = 720
        } else if (this.settings.videoSize == "1080p") {
            width = 1920
            height = 1080
        } else if (this.settings.videoSize == "1440p") {
            width = 2560
            height = 1440
        } else if (this.settings.videoSize == "4k") {
            width = 3840
            height = 2160
        } else if (this.settings.videoSize == "custom") {
            width = this.settings.videoSizeCustom.width
            height = this.settings.videoSizeCustom.height
        } else { // native
            width = viewerScreenSize[0]
            height = viewerScreenSize[1]
        }

        this.sendWsMessage({
            AuthenticateAndInit: {
                credentials: this.api.credentials,
                host_id: this.hostId,
                app_id: this.appId,
                bitrate: this.settings.bitrate,
                packet_size: this.settings.packetSize,
                fps: this.settings.fps,
                width,
                height,
                video_sample_queue_size: this.settings.videoSampleQueueSize,
                play_audio_local: this.settings.playAudioLocal,
                audio_sample_queue_size: this.settings.audioSampleQueueSize,
                video_supported_formats: createSupportedVideoFormatsBits(supported_video_formats),
                video_colorspace: "Rec709", // TODO <---
                video_color_range_full: true, // TODO <---
            }
        })

        // Configure web rtc
        this.peer = new RTCPeerConnection()
        this.peer.addEventListener("error", this.onError.bind(this))

        this.peer.addEventListener("negotiationneeded", this.onNegotiationNeeded.bind(this))
        this.peer.addEventListener("icecandidate", this.onIceCandidate.bind(this))

        this.peer.addEventListener("track", this.onTrack.bind(this))
        this.peer.addEventListener("datachannel", this.onDataChannel.bind(this))
        this.peer.addEventListener("iceconnectionstatechange", this.onConnectionStateChange.bind(this))

        const streamInputConfig = defaultStreamInputConfig()
        Object.assign(streamInputConfig, {
            controllerConfig: settings.controllerConfig
        })
        this.input = new StreamInput(this.peer, streamInputConfig)
    }

    private async onMessage(message: StreamServerMessage | StreamServerGeneralMessage) {
        if (typeof message == "string") {
            const event: InfoEvent = new CustomEvent("stream-info", {
                detail: { type: "error", message }
            })

            this.eventTarget.dispatchEvent(event)
        } else if ("StageStarting" in message) {
            const event: InfoEvent = new CustomEvent("stream-info", {
                detail: { type: "stageStarting", stage: message.StageStarting.stage }
            })

            this.eventTarget.dispatchEvent(event)
        } else if ("StageComplete" in message) {
            const event: InfoEvent = new CustomEvent("stream-info", {
                detail: { type: "stageComplete", stage: message.StageComplete.stage }
            })

            this.eventTarget.dispatchEvent(event)
        } else if ("StageFailed" in message) {
            const event: InfoEvent = new CustomEvent("stream-info", {
                detail: { type: "stageFailed", stage: message.StageFailed.stage, errorCode: message.StageFailed.error_code }
            })

            this.eventTarget.dispatchEvent(event)
        } else if ("ConnectionTerminated" in message) {
            const event: InfoEvent = new CustomEvent("stream-info", {
                detail: { type: "connectionTerminated", errorCode: message.ConnectionTerminated.error_code }
            })

            this.eventTarget.dispatchEvent(event)
        } else if ("ConnectionStatusUpdate" in message) {
            const event: InfoEvent = new CustomEvent("stream-info", {
                detail: { type: "connectionStatus", status: message.ConnectionStatusUpdate.status }
            })

            this.eventTarget.dispatchEvent(event)
        } else if ("UpdateApp" in message) {
            const event: InfoEvent = new CustomEvent("stream-info", {
                detail: { type: "app", app: message.UpdateApp.app }
            })

            this.eventTarget.dispatchEvent(event)
        } else if ("ConnectionComplete" in message) {
            const capabilities = message.ConnectionComplete.capabilities

            const event: InfoEvent = new CustomEvent("stream-info", {
                detail: { type: "connectionComplete", capabilities }
            })

            this.eventTarget.dispatchEvent(event)

            this.input.setCapabilities(capabilities)
        }
        // -- WebRTC Config
        else if ("WebRtcConfig" in message) {
            const iceServers = message.WebRtcConfig.ice_servers

            this.peer.setConfiguration({
                iceServers,
            })
        }
        // -- Signaling
        else if ("Signaling" in message) {
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
        event.receiver.jitterBufferTarget = 0

        // TODO: remove debug
        console.log(event)
        const stream = event.streams[0]
        if (stream) {
            stream.getTracks().forEach(track => this.mediaStream.addTrack(track))
        }
        // this.mediaStream.addTrack(event.track)
    }
    private onConnectionStateChange(event: Event) {
        // TODO: remove
        console.log(this.peer.connectionState)
    }

    private onDataChannel(event: RTCDataChannelEvent) {
        if (event.channel.label == "general") {
            event.channel.addEventListener("message", this.onGeneralDataChannelMessage.bind(this))
        }
    }
    private async onGeneralDataChannelMessage(event: MessageEvent) {
        const data = event.data

        if (typeof data != "string") {
            return
        }

        let message = JSON.parse(data)
        await this.onMessage(message)
    }

    // -- Raw Web Socket stuff
    private wsSendBuffer: Array<string> = []

    private onWsOpen() {
        for (const raw of this.wsSendBuffer.splice(0, this.wsSendBuffer.length)) {
            this.ws.send(raw)
        }
    }

    private sendWsMessage(message: StreamClientMessage) {
        const raw = JSON.stringify(message)
        if (this.ws.readyState == WebSocket.OPEN) {
            this.ws.send(raw)
        } else {
            this.wsSendBuffer.push(raw)
        }
    }

    private async onRawWsMessage(event: MessageEvent) {
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
    addInfoListener(listener: InfoEventListener) {
        this.eventTarget.addEventListener("stream-info", listener as EventListenerOrEventListenerObject)
    }
    removeInfoListener(listener: InfoEventListener) {
        this.eventTarget.removeEventListener("stream-info", listener as EventListenerOrEventListenerObject)
    }

    getMediaStream(): MediaStream {
        return this.mediaStream
    }

    getInput(): StreamInput {
        return this.input
    }
}