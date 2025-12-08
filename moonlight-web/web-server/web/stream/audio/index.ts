import { Component } from "../../component/index.js"

export type AudioPlayerSetup = {

}

export type AudioPlayer = StreamAudioPlayer
interface AudioPlayerBase extends Component {
    readonly implementationName: string
    readonly type: string

    setup(setup: AudioPlayerSetup): void
    cleanup(): void

    onUserInteraction(): void
}

export interface StreamAudioPlayer extends AudioPlayerBase {
    readonly type: "stream"

    setTrack(track: MediaStreamTrack): void
}

export type AudioDecodeUnit = {
    timestampMicroseconds: number
    durationMicroseconds: number
    data: ArrayBuffer
}

export interface DataAudioPlayer extends AudioPlayerBase {
    readonly type: "moonlightdata"

    // Data like https://github.com/moonlight-stream/moonlight-common-c/blob/b126e481a195fdc7152d211def17190e3434bcce/src/Limelight.h#L356
    decode_and_play(unit: AudioDecodeUnit): void
}