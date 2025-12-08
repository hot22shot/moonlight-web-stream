import { AudioPlayerSetup, StreamAudioPlayer } from ".";

export class AudioElementPlayer implements StreamAudioPlayer {
    implementationName: string = "audio_element"
    type: "stream" = "stream"

    static isBrowserSupported(): boolean {
        // TODO
        return true
    }

    private audioElement = document.createElement("audio")
    private oldTrack: MediaStreamTrack | null = null
    private stream = new MediaStream()

    constructor() {
        this.audioElement.classList.add("audio-stream")
        this.audioElement.preload = "none"
        this.audioElement.controls = false
        this.audioElement.autoplay = true
        this.audioElement.muted = true
        this.audioElement.srcObject = this.stream
    }

    setup(_setup: AudioPlayerSetup): void { }
    cleanup(): void {
        if (this.oldTrack) {
            this.stream.removeTrack(this.oldTrack)
            this.oldTrack = null
        }
        this.audioElement.srcObject = null
    }

    setTrack(track: MediaStreamTrack): void {
        if (this.oldTrack) {
            this.stream.removeTrack(this.oldTrack)
            this.oldTrack = null
        }

        this.stream.addTrack(track)
        this.oldTrack = track
    }

    onUserInteraction(): void {
        this.audioElement.muted = false
    }

    mount(parent: HTMLElement): void {
        parent.appendChild(this.audioElement)
    }
    unmount(parent: HTMLElement): void {
        parent.removeChild(this.audioElement)
    }
}