import { AudioPlayerSetup, StreamAudioPlayer } from "./index.js";

export class AudioElementPlayer extends StreamAudioPlayer {

    static isBrowserSupported(): boolean {
        return "HTMLAudioElement" in window && "srcObject" in HTMLAudioElement.prototype
    }

    private audioElement = document.createElement("audio")
    private oldTrack: MediaStreamTrack | null = null
    private stream = new MediaStream()

    constructor() {
        super("audio_element")

        this.audioElement.classList.add("audio-stream")
        this.audioElement.preload = "none"
        this.audioElement.controls = false
        this.audioElement.autoplay = true
        this.audioElement.muted = true
        this.audioElement.srcObject = this.stream
    }

    setup(_setup: AudioPlayerSetup) {
        return true
    }
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