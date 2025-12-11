declare global {
    interface Navigator {
        // Keyboard Lock: https://developer.mozilla.org/en-US/docs/Web/API/Keyboard/lock
        keyboard: {
            lock(): Promise<void>;
            unlock(): void;
        };
    }

    // MediaStreamTrackProcessor: https://developer.mozilla.org/en-US/docs/Web/API/MediaStreamTrackProcessor
    interface MediaStreamTrackProcessor {
        readonly readable: ReadableStream<VideoFrame>
    }

    var MediaStreamTrackProcessor: {
        prototype: MediaStreamTrackProcessor
        new(options: { track: MediaStreamTrack, maxBufferSize?: number }): MediaStreamTrackProcessor
        new(): MediaStreamTrackProcessor
    }
}


export { };