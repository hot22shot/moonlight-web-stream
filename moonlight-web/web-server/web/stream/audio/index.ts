import { Component } from "../../component/index.js"
import { Pipe } from "../pipeline/index.js"

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

export interface TrackAudioPlayer extends Pipe {
    // static readonly type: "audiotrack"

    setTrack(track: MediaStreamTrack): void
}

export type AudioDecodeUnit = {
    timestampMicroseconds: number
    durationMicroseconds: number
    data: ArrayBuffer
}

export interface DataAudioPlayer extends Pipe {
    // static readonly type: "audiodata"

    // Data like https://github.com/moonlight-stream/moonlight-common-c/blob/b126e481a195fdc7152d211def17190e3434bcce/src/Limelight.h#L356
    decodeAndPlay(unit: AudioDecodeUnit): void
}

export interface SampleAudioPlayer extends Pipe {
    // static readonly type: "audiosample"

    submitSample(sample: AudioData): void
}