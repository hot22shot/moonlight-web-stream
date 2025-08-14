import { Component, ComponentEvent } from "./index.js";

export type StreamSettings = {
    bitrate: number
    packetSize: number
    videoSize?: {
        width: number,
        height: number,
    },
    fps: number
}

export function defaultStreamSettings(): StreamSettings {
    return {
        bitrate: 5000,
        packetSize: 4096,
        fps: 60,
    }
}

export function getLocalStreamSettings(): StreamSettings | null {
    let settings = null
    try {
        const settingsLoadedJson = localStorage.getItem("mlSettings")
        if (settingsLoadedJson == null) {
            return null
        }

        const settingsLoaded = JSON.parse(settingsLoadedJson)

        settings = defaultStreamSettings()
        Object.assign(settings, settingsLoaded)
    } catch (e) {
        localStorage.removeItem("mlSettings")
    }
    return settings
}
export function setLocalStreamSettings(settings?: StreamSettings) {
    localStorage.setItem("mlSettings", JSON.stringify(settings))
}

export type StreamSettingsChangeListener = (event: ComponentEvent<StreamSettingsComponent>) => void

type Input = {
    div: HTMLDivElement
    label: HTMLLabelElement
    input: HTMLInputElement
}
function createInput(): Input {
    return {
        div: document.createElement("div"),
        label: document.createElement("label"),
        input: document.createElement("input")
    }
}

export class StreamSettingsComponent implements Component {

    private divElement: HTMLDivElement = document.createElement("div")

    private bitrate: Input = createInput()
    private packetSize: Input = createInput()
    private fps: Input = createInput()

    private videoSizeEnabled = createInput()
    private videoSizeWidth = createInput()
    private videoSizeHeight = createInput()

    constructor(settings?: StreamSettings) {
        const defaultSettings = defaultStreamSettings()

        // Root div
        this.divElement.classList.add("settings")

        // Bitrate
        this.configureInput(this.bitrate, "bitrate", "Bitrate", "number", defaultSettings.bitrate.toString(), settings?.bitrate.toString(), {
            step: "100",
        })

        // Packet Size
        this.configureInput(this.packetSize, "packetSize", "Packet Size", "number", defaultSettings.packetSize.toString(), settings?.packetSize.toString(), {
            step: "100",
        })

        // Fps
        this.configureInput(this.fps, "fps", "Fps", "number", defaultSettings.fps.toString(), settings?.fps.toString(), {
            step: "5",
        })

        // Video Size
        this.configureInput(this.videoSizeEnabled, "videoSizeEnabled", "Fixed Video Size", "checkbox", "", settings?.videoSize ? "on" : undefined)
        this.configureInput(this.videoSizeWidth, "videoSizeWidth", "Video Width", "number", "1920", settings?.videoSize ? settings.videoSize.width.toString() : undefined)
        this.configureInput(this.videoSizeHeight, "videoSizeHeight", "Video Height", "number", "1080", settings?.videoSize ? settings.videoSize.height.toString() : undefined)

        this.onSettingsChange()
    }

    private configureInput(
        input: Input, internalName: string, displayName: string, type: string, defaultValue: string, value?: string | null,
        extra?: { step?: string }
    ) {
        input.label.htmlFor = internalName
        input.label.innerText = displayName
        input.div.appendChild(input.label)

        input.input.name = internalName
        input.input.type = type
        input.input.defaultValue = defaultValue
        if (value != null) {
            input.input.value = value
        }
        if (extra && extra.step != undefined) {
            input.input.step = extra.step
        }
        input.input.addEventListener("change", this.onSettingsChange.bind(this))
        input.div.appendChild(input.input)

        this.divElement.appendChild(input.div)
    }

    private onSettingsChange() {
        if (this.videoSizeEnabled.input.checked) {
            this.videoSizeWidth.input.disabled = false
            this.videoSizeHeight.input.disabled = false
        } else {
            this.videoSizeWidth.input.disabled = true
            this.videoSizeHeight.input.disabled = true
        }

        this.divElement.dispatchEvent(new ComponentEvent("ml-settingschange", this))
    }

    addChangeListener(listener: StreamSettingsChangeListener) {
        this.divElement.addEventListener("ml-settingschange", listener as any)
    }
    removeChangeListener(listener: StreamSettingsChangeListener) {
        this.divElement.removeEventListener("ml-settingschange", listener as any)
    }

    getStreamSettings(): StreamSettings {
        const settings = defaultStreamSettings()

        settings.bitrate = parseInt(this.bitrate.input.value)
        settings.packetSize = parseInt(this.packetSize.input.value)
        settings.fps = parseInt(this.fps.input.value)
        if (this.videoSizeEnabled.input.checked) {
            settings.videoSize = {
                width: parseInt(this.videoSizeWidth.input.value),
                height: parseInt(this.videoSizeHeight.input.value)
            }
        }

        return settings
    }

    mount(parent: HTMLElement): void {
        parent.appendChild(this.divElement)
    }
    unmount(parent: HTMLElement): void {
        parent.removeChild(this.divElement)
    }
}