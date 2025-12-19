import { AudioDecodeUnit, AudioPlayerSetup, DataAudioPlayer, SampleAudioPlayer } from "./index.js";

export class AudioDecoderPipe<T extends SampleAudioPlayer> extends DataAudioPlayer {

    static isBrowserSupported(): boolean {
        // TODO: allow streaming without audio
        return "AudioDecoder" in window
    }

    private base: T

    private decoder: AudioDecoder

    constructor(base: T) {
        super(`audio_decoder -> ${base.implementationName}`)

        this.base = base

        this.decoder = new AudioDecoder({
            error: this.onError.bind(this),
            output: this.onOutput.bind(this)
        })
    }

    private onError(error: any) {
        // TODO: use logger
        console.error(error)
    }

    private onOutput(sample: AudioData) {
        this.base.submitSample(sample)
    }


    setup(setup: AudioPlayerSetup): void {
        this.base.setup(setup)

        this.decoder.configure({
            codec: "opus",
            numberOfChannels: setup.channels,
            sampleRate: setup.sampleRate
        })
    }

    decodeAndPlay(unit: AudioDecodeUnit): void {
        const chunk = new EncodedAudioChunk({
            type: "key", // TODO: there are audio key and delta frame
            data: unit.data,
            timestamp: unit.timestampMicroseconds,
            duration: unit.durationMicroseconds,
            // We should be allowed to transfer because this data won't be used in the future
            transfer: [unit.data]
        })

        this.decoder.decode(chunk)
    }

    cleanup(): void {
        this.base.cleanup()

        this.decoder.close()
    }

    onUserInteraction(): void {
        this.base.onUserInteraction()
    }
    mount(parent: HTMLElement): void {
        this.base.mount(parent)
    }
    unmount(parent: HTMLElement): void {
        this.base.unmount(parent)
    }

}