import { StreamSignalingMessage, TransportChannelId } from "../../api_bindings.js";
import { DataTransportChannel, Transport, TRANSPORT_CHANNEL_OPTIONS, TransportAudioSetup, TransportChannel, TransportChannelIdKey, TransportChannelIdValue, TransportVideoSetup, AudioTrackTransportChannel, VideoTrackTransportChannel, TrackTransportChannel } from "./index.js";

export class WebRTCTransport implements Transport {
    implementationName: string = "webrtc"

    ondebug: ((message: string, type?: "fatal" | "recover") => void) | null = null
    private debugLog(message: string, type?: "fatal" | "recover") {
        if (this.ondebug) {
            this.ondebug(message, type)
        }
    }

    private peer: RTCPeerConnection | null = null

    constructor() { }

    async initPeer(configuration?: RTCConfiguration) {
        this.debugLog(`Creating Client Peer`)

        if (this.peer) {
            this.debugLog(`Cannot create Peer because a Peer already exists`)
            return
        }

        // Configure web rtc
        this.peer = new RTCPeerConnection(configuration)
        this.peer.addEventListener("error", this.onError.bind(this))

        this.peer.addEventListener("negotiationneeded", this.onNegotiationNeeded.bind(this))
        this.peer.addEventListener("icecandidate", this.onIceCandidate.bind(this))

        this.peer.addEventListener("connectionstatechange", this.onConnectionStateChange.bind(this))
        this.peer.addEventListener("iceconnectionstatechange", this.onIceConnectionStateChange.bind(this))
        this.peer.addEventListener("icegatheringstatechange", this.onIceGatheringStateChange.bind(this))

        this.peer.addEventListener("track", this.onTrack.bind(this))

        this.initChannels()

        // Maybe we already received data
        if (this.remoteDescription) {
            await this.handleRemoteDescription(this.remoteDescription)
        } else {
            await this.onNegotiationNeeded()
        }
        await this.tryDequeueIceCandidates()
    }

    private onError(event: Event) {
        this.debugLog(`Web Socket or WebRtcPeer Error`)

        console.error(`Web Socket or WebRtcPeer Error`, event)
    }

    onsendmessage: ((message: StreamSignalingMessage) => void) | null = null
    private sendMessage(message: StreamSignalingMessage) {
        if (this.onsendmessage) {
            this.onsendmessage(message)
        } else {
            this.debugLog("Failed to call onicecandidate because no handler is set")
        }
    }
    async onReceiveMessage(message: StreamSignalingMessage) {
        if ("Description" in message) {
            const description = message.Description;
            await this.handleRemoteDescription({
                type: description.ty as RTCSdpType,
                sdp: description.sdp
            })
        } else if ("AddIceCandidate" in message) {
            const candidate = message.AddIceCandidate
            await this.addIceCandidate({
                candidate: candidate.candidate,
                sdpMid: candidate.sdp_mid,
                sdpMLineIndex: candidate.sdp_mline_index,
                usernameFragment: candidate.username_fragment
            })
        }
    }

    private async onNegotiationNeeded() {
        // We're polite
        if (!this.peer) {
            this.debugLog("OnNegotiationNeeded without a peer")
            return
        }

        await this.peer.setLocalDescription()
        const localDescription = this.peer.localDescription
        if (!localDescription) {
            this.debugLog("Failed to set local description in OnNegotiationNeeded")
            return
        }

        this.debugLog(`OnNegotiationNeeded: Sending local description: ${localDescription.type}`)
        this.sendMessage({
            Description: {
                ty: localDescription.type,
                sdp: localDescription.sdp ?? ""
            }
        })
    }

    private remoteDescription: RTCSessionDescriptionInit | null = null
    private async handleRemoteDescription(sdp: RTCSessionDescriptionInit | null) {
        this.debugLog(`Received remote description: ${sdp?.type}`)

        this.remoteDescription = sdp
        if (!this.peer) {
            return
        }

        if (this.remoteDescription) {
            await this.peer.setRemoteDescription(this.remoteDescription)

            if (this.remoteDescription.type == "offer") {
                await this.peer.setLocalDescription()
                const localDescription = this.peer.localDescription
                if (!localDescription) {
                    this.debugLog("Peer didn't have a localDescription whilst receiving an offer and trying to answer")
                    return
                }

                this.debugLog(`Responding to offer description: ${localDescription.type}`)
                this.sendMessage({
                    Description: {
                        ty: localDescription.type,
                        sdp: localDescription.sdp ?? ""
                    }
                })
            }

            this.remoteDescription = null
        }
    }

    private onIceCandidate(event: RTCPeerConnectionIceEvent) {
        if (event.candidate) {
            const candidate = event.candidate.toJSON()
            this.debugLog(`Sending ice candidate: ${candidate.candidate}`)

            this.sendMessage({
                AddIceCandidate: {
                    candidate: candidate.candidate ?? "",
                    sdp_mid: candidate.sdpMid ?? null,
                    sdp_mline_index: candidate.sdpMLineIndex ?? null,
                    username_fragment: candidate.usernameFragment ?? null
                }
            })
        } else {
            this.debugLog("No new ice candidates")
        }
    }

    private iceCandidates: Array<RTCIceCandidateInit> = []
    private async addIceCandidate(candidate: RTCIceCandidateInit) {
        this.debugLog(`Received ice candidate: ${candidate.candidate}`)

        if (!this.peer) {
            this.debugLog("Buffering ice candidate")
            this.iceCandidates.push(candidate)
            return
        }
        await this.tryDequeueIceCandidates()

        await this.peer.addIceCandidate(candidate)
    }
    private async tryDequeueIceCandidates() {
        if (!this.peer) {
            this.debugLog("called tryDequeueIceCandidates without a peer")
            return
        }

        for (const candidate of this.iceCandidates) {
            await this.peer.addIceCandidate(candidate)
        }
        this.iceCandidates.length = 0
    }

    private onConnectionStateChange() {
        if (!this.peer) {
            this.debugLog("OnConnectionStateChange without a peer")
            return
        }

        let type: undefined | "fatal" | "recover" = undefined

        if (this.peer.connectionState == "connected") {
            type = "recover"
        } else if ((this.peer.connectionState == "failed" || this.peer.connectionState == "disconnected") && this.peer.iceGatheringState == "complete") {
            type = "fatal"
        }

        this.debugLog(`Changing Peer State to ${this.peer.connectionState}`, type)
    }
    private onIceConnectionStateChange() {
        if (!this.peer) {
            this.debugLog("OnIceConnectionStateChange without a peer")
            return
        }
        this.debugLog(`Changing Peer Ice State to ${this.peer.iceConnectionState}`)
    }
    private onIceGatheringStateChange() {
        if (!this.peer) {
            this.debugLog("OnIceGatheringStateChange without a peer")
            return
        }
        this.debugLog(`Changing Peer Ice Gathering State to ${this.peer.iceGatheringState}`)
    }

    private channels: Array<TransportChannel | null> = []
    private initChannels() {
        if (!this.peer) {
            this.debugLog("Failed to initialize channel without peer")
            return
        }
        if (this.channels.length > 0) {
            this.debugLog("Already initialized channels")
            return
        }

        for (const channelRaw in TRANSPORT_CHANNEL_OPTIONS) {
            const channel = channelRaw as TransportChannelIdKey
            const options = TRANSPORT_CHANNEL_OPTIONS[channel]

            if (channel == "HOST_VIDEO") {
                const channel: VideoTrackTransportChannel = new WebRTCInboundTrackTransportChannel<"videotrack">("videotrack", "video", this.videoTrackHolder)
                this.channels[TransportChannelId.HOST_VIDEO] = channel
                continue
            }
            if (channel == "HOST_AUDIO") {
                const channel: AudioTrackTransportChannel = new WebRTCInboundTrackTransportChannel<"audiotrack">("audiotrack", "audio", this.audioTrackHolder)
                this.channels[TransportChannelId.HOST_AUDIO] = channel
                continue
            }

            const id = TransportChannelId[channel]
            const dataChannel = this.peer.createDataChannel(channel.toLowerCase(), {
                // TODO: use id
                // id,
                // negotiated: true,
                ordered: options.ordered,
                maxRetransmits: options.reliable ? undefined : 0
            })

            this.channels[id] = new WebRTCDataTransportChannel(channel, dataChannel)
        }
    }

    private videoTrackHolder: TrackHolder = { ontrack: null, track: null }
    private videoReceiver: RTCRtpReceiver | null = null

    private audioTrackHolder: TrackHolder = { ontrack: null, track: null }

    private onTrack(event: RTCTrackEvent) {
        const track = event.track

        if ("playoutDelayHint" in event.receiver) {
            event.receiver.playoutDelayHint = 0
        } else {
            this.debugLog(`playoutDelayHint not supported in receiver: ${event.receiver.track.label}`)
        }

        if (track.kind == "video") {
            this.videoReceiver = event.receiver
        }

        this.debugLog(`Adding receiver: ${track.kind}, ${track.id}, ${track.label}`)

        if (track.kind == "video") {
            event.receiver.jitterBufferTarget = 0

            if ("contentHint" in track) {
                track.contentHint = "motion"
            }

            this.videoTrackHolder.track = track
            if (!this.videoTrackHolder.ontrack) {
                throw "No video track listener registered!"
            }
            this.videoTrackHolder.ontrack()
        } else if (track.kind == "audio") {
            // no jitterBufferTarget because Audio cracks, which we don't want

            this.audioTrackHolder.track = track
            if (!this.audioTrackHolder.ontrack) {
                throw "No audio track listener registered!"
            }
            this.audioTrackHolder.ontrack()
        }
    }

    async setupHostVideo(_setup: TransportVideoSetup): Promise<void> {
        if (!this.peer) {
            this.debugLog("Failed to setup video without peer")
            return
        }
    }

    async setupHostAudio(_setup: TransportAudioSetup): Promise<void> { }

    getChannel(id: TransportChannelIdValue): TransportChannel {
        const channel = this.channels[id]
        if (!channel) {
            throw `Failed to get channel because it is not yet initialized, Id: ${id}`
        }

        return channel
    }

    onconnected: (() => void) | null = null
    ondisconnected: (() => void) | null = null

    onclose: (() => void) | null = null
    async close(): Promise<void> {
        // TODO: when is it actually closed? await?
        this.peer?.close()
    }

    async getStats(): Promise<Record<string, string>> {
        const statsData: Record<string, string> = {}

        if (!this.videoReceiver) {
            return {}
        }
        const stats = await this.videoReceiver.getStats()

        console.debug("----------------- raw video stats -----------------")
        for (const [key, value] of stats.entries()) {
            console.debug("raw video stats", key, value)

            if ("decoderImplementation" in value && value.decoderImplementation != null) {
                statsData.decoderImplementation = value.decoderImplementation
            }
            if ("frameWidth" in value && value.frameWidth != null) {
                statsData.videoWidth = value.frameWidth
            }
            if ("frameHeight" in value && value.frameHeight != null) {
                statsData.videoHeight = value.frameHeight
            }
            if ("framesPerSecond" in value && value.framesPerSecond != null) {
                statsData.webrtcFps = value.framesPerSecond
            }

            if ("jitterBufferDelay" in value && value.jitterBufferDelay != null) {
                statsData.webrtcJitterBufferDelayMs = value.jitterBufferDelay
            }
            if ("jitterBufferTargetDelay" in value && value.jitterBufferTargetDelay != null) {
                statsData.webrtcJitterBufferTargetDelayMs = value.jitterBufferTargetDelay
            }
            if ("jitterBufferMinimumDelay" in value && value.jitterBufferMinimumDelay != null) {
                statsData.webrtcJitterBufferMinimumDelayMs = value.jitterBufferMinimumDelay
            }
            if ("jitter" in value && value.jitter != null) {
                statsData.webrtcJitterMs = value.jitter
            }
            if ("totalDecodeTime" in value && value.totalDecodeTime != null) {
                statsData.webrtcTotalDecodeTimeMs = value.totalDecodeTime
            }
            if ("totalAssemblyTime" in value && value.totalAssemblyTime != null) {
                statsData.webrtcTotalAssemblyTimeMs = value.totalAssemblyTime
            }
            if ("totalProcessingDelay" in value && value.totalProcessingDelay != null) {
                statsData.webrtcTotalProcessingDelayMs = value.totalProcessingDelay
            }
            if ("packetsReceived" in value && value.packetsReceived != null) {
                statsData.webrtcPacketsReceived = value.packetsReceived
            }
            if ("packetsLost" in value && value.packetsLost != null) {
                statsData.webrtcPacketsLost = value.packetsLost
            }
            if ("framesDropped" in value && value.framesDropped != null) {
                statsData.webrtcFramesDropped = value.framesDropped
            }
            if ("keyFramesDecoded" in value && value.keyFramesDecoded != null) {
                statsData.webrtcKeyFramesDecoded = value.keyFramesDecoded
            }
        }

        return statsData
    }
}

type TrackHolder = {
    ontrack: (() => void) | null
    track: MediaStreamTrack | null
}

// This receives track data
class WebRTCInboundTrackTransportChannel<T extends string> implements TrackTransportChannel {
    type: T

    canReceive: boolean = true
    canSend: boolean = false

    private label: string
    private trackHolder: TrackHolder

    constructor(type: T, label: string, trackHolder: TrackHolder) {
        this.type = type
        this.label = label
        this.trackHolder = trackHolder

        this.trackHolder.ontrack = this.onTrack.bind(this)
    }
    setTrack(_track: MediaStreamTrack | null): void {
        throw "WebRTCInboundTrackTransportChannel cannot addTrack"
    }

    private onTrack() {
        const track = this.trackHolder.track
        if (!track) {
            throw "WebRTC TrackHolder.track is null!"
        }

        for (const listener of this.trackListeners) {
            listener(track)
        }
    }

    private trackListeners: Array<(track: MediaStreamTrack) => void> = []
    addTrackListener(listener: (track: MediaStreamTrack) => void): void {
        if (this.trackHolder.track) {
            listener(this.trackHolder.track)
        }
        this.trackListeners.push(listener)
    }
    removeTrackListener(listener: (track: MediaStreamTrack) => void): void {
        const index = this.trackListeners.indexOf(listener)
        if (index != -1) {
            this.trackListeners.splice(index, 1)
        }
    }
}

class WebRTCDataTransportChannel implements DataTransportChannel {
    type: "data" = "data"

    canReceive: boolean = true
    canSend: boolean = true

    private label: string
    private channel: RTCDataChannel

    constructor(label: string, channel: RTCDataChannel) {
        this.label = label
        this.channel = channel

        this.channel.addEventListener("message", this.onMessage.bind(this))
    }

    private sendQueue: Array<ArrayBuffer> = []
    send(message: ArrayBuffer): void {
        console.debug(this.label, message)

        if (this.channel.readyState != "open") {
            console.debug(`Tried sending packet to ${this.label} with readyState ${this.channel.readyState}. Buffering it for the future.`)
            this.sendQueue.push(message)
        } else {
            this.tryDequeueSendQueue()
            this.channel.send(message)
        }
    }
    private tryDequeueSendQueue() {
        for (const message of this.sendQueue) {
            this.channel.send(message)
        }
        this.sendQueue.length = 0
    }

    private onMessage(event: MessageEvent) {
        const data = event.data
        if (!(data instanceof ArrayBuffer)) {
            console.warn(`received text data on webrtc channel ${this.label}`)
            return
        }

        for (const listener of this.receiveListeners) {
            listener(event.data)
        }
    }
    private receiveListeners: Array<(data: ArrayBuffer) => void> = []
    addReceiveListener(listener: (data: ArrayBuffer) => void): void {
        this.receiveListeners.push(listener)
    }
    removeReceiveListener(listener: (data: ArrayBuffer) => void): void {
        const index = this.receiveListeners.indexOf(listener)
        if (index != -1) {
            this.receiveListeners.splice(index, 1)
        }
    }
    estimatedBufferedBytes(): number {
        return this.channel.bufferedAmount
    }
}