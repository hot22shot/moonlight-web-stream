import { Component } from "../../component/index.js"

export type AudioPlayerSetup = {

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

export abstract class StreamAudioPlayer extends AudioPlayer {
    abstract setTrack(track: MediaStreamTrack): void
}

export type AudioDecodeUnit = {
    timestampMicroseconds: number
    durationMicroseconds: number
    data: ArrayBuffer
}

export interface DataAudioPlayer extends AudioPlayer {
    readonly type: "moonlightdata"

    // Data like https://github.com/moonlight-stream/moonlight-common-c/blob/b126e481a195fdc7152d211def17190e3434bcce/src/Limelight.h#L356
    decode_and_play(unit: AudioDecodeUnit): void
}