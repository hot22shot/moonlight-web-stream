import { Component } from "../../component/index.js"

export type AudioPlayerSetup = {
    channels: number
    sampleRate: number
}

export abstract class AudioPlayer implements Component {
    readonly implementationName: string

    constructor(implementationName: string) {
        this.implementationName = implementationName
    }

    abstract setup(setup: AudioPlayerSetup): void
    abstract cleanup(): void

    abstract onUserInteraction(): void

    abstract mount(parent: HTMLElement): void
    abstract unmount(parent: HTMLElement): void
}

export abstract class TrackAudioPlayer extends AudioPlayer {
    static readonly type: "audiotrack"

    abstract setTrack(track: MediaStreamTrack): void
}

export type AudioDecodeUnit = {
    timestampMicroseconds: number
    durationMicroseconds: number
    data: ArrayBuffer
}

export abstract class DataAudioPlayer extends AudioPlayer {
    static readonly type: "data"

    // Data like https://github.com/moonlight-stream/moonlight-common-c/blob/b126e481a195fdc7152d211def17190e3434bcce/src/Limelight.h#L356
    abstract decodeAndPlay(unit: AudioDecodeUnit): void
}

export abstract class SampleAudioPlayer extends AudioPlayer {
    static readonly type: "audiosample"

    abstract submitSample(sample: AudioData): void
}