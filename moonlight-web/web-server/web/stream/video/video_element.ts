import { getStreamRectCorrected, TrackVideoRenderer, VideoRendererSetup } from "./index.js";

export class VideoElementRenderer implements TrackVideoRenderer {
    implementationName: string = "video_element"
    type: "stream" = "stream"

    static isBrowserSupported(): boolean {
        // TODO
        return true
    }

    private videoElement = document.createElement("video")
    private oldTrack: MediaStreamTrack | null = null
    private stream = new MediaStream()

    private size: [number, number] | null = null

    constructor() {
        this.videoElement.classList.add("video-stream")
        this.videoElement.preload = "none"
        this.videoElement.controls = false
        this.videoElement.autoplay = true
        this.videoElement.disablePictureInPicture = true
        this.videoElement.playsInline = true
        this.videoElement.muted = true
        this.videoElement.srcObject = this.stream
    }

    setup(setup: VideoRendererSetup): void {
        this.size = [setup.width, setup.height]
    }
    cleanup(): void {
        if (this.oldTrack) {
            this.stream.removeTrack(this.oldTrack)
        }
        this.videoElement.srcObject = null
    }

    setTrack(track: MediaStreamTrack): void {
        if (this.oldTrack) {
            this.stream.removeTrack(this.oldTrack)
        }

        this.stream.addTrack(track)
        this.oldTrack = track
    }

    mount(parent: HTMLElement): void {
        parent.appendChild(this.videoElement)
    }
    unmount(parent: HTMLElement): void {
        parent.removeChild(this.videoElement)
    }

    onUserInteraction(): void {
        if (this.videoElement.paused) {
            this.videoElement.play().then(() => {
                // Playing
            }).catch(error => {
                console.error(`Failed to play videoElement: ${error.message || error}`);
            })
        }
    }
    getStreamRect(): DOMRect {
        if (!this.size) {
            return new DOMRect()
        }

        return getStreamRectCorrected(this.videoElement.getBoundingClientRect(), this.size)
    }
}