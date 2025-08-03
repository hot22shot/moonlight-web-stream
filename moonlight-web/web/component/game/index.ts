import { Component } from "../index.js";
import { Api, apiGetAppImage } from "../../api.js";
import { App } from "../../api_bindings.js";
import { setContextMenu } from "../context_menu.js";
import { showMessage } from "../modal.js";
import { APP_NOT_FOUND as APP_NO_IMAGE } from "../../resources/index.js";

export class Game implements Component {
    private api: Api

    private hostId: number
    private appId: number

    private mounted: number = 0
    private divElement: HTMLDivElement = document.createElement("div")

    private imageBlob: Blob | null = null
    private imageBlobUrl: string | null = null
    private imageElement: HTMLImageElement = document.createElement("img")

    private imageOverlayElement: HTMLImageElement = document.createElement("img")

    private cache: App

    constructor(api: Api, hostId: number, appId: number, game: App) {
        this.api = api

        this.hostId = hostId
        this.appId = appId

        this.cache = game

        // Configure image
        this.imageElement.classList.add("app-image")
        this.imageElement.src = APP_NO_IMAGE

        this.forceLoadImage()

        // Configure image overlay
        this.imageOverlayElement.classList.add("app-image-overlay")

        // Append elements
        this.divElement.appendChild(this.imageElement)
        this.divElement.appendChild(this.imageOverlayElement)

        this.divElement.addEventListener("click", this.onClick.bind(this))
        this.divElement.addEventListener("contextmenu", this.onContextMenu.bind(this))

        this.updateCache(game)
    }

    async forceLoadImage() {
        this.imageBlob = await apiGetAppImage(this.api, {
            host_id: this.hostId,
            app_id: this.appId
        })

        this.updateImage()
    }
    private updateImage() {
        // generate and set url
        if (this.imageBlob && !this.imageBlobUrl && this.mounted > 0) {
            this.imageBlobUrl = URL.createObjectURL(this.imageBlob)

            this.imageElement.classList.add("app-image-loaded")
            this.imageElement.src = this.imageBlobUrl
        }

        // revoke url
        if (this.imageBlobUrl && this.mounted <= 0) {
            URL.revokeObjectURL(this.imageBlobUrl)

            this.imageElement.classList.remove("app-image-loaded")
            this.imageElement.src = ""
        }
    }

    updateCache(cache: App) {
        this.cache = cache
    }

    private async onClick() {
        // TODO
        await showMessage("NOT YET IMPLEMENTED: STREAMING")
    }

    private onContextMenu(event: MouseEvent) {
        const elements = []

        elements.push({
            name: "Show Details",
            callback: this.showDetails.bind(this),
        })

        setContextMenu(event, {
            elements
        })
    }

    private async showDetails() {
        const app = this.cache

        await showMessage(
            `Title: ${app.title}\n` +
            `Id: ${app.app_id}\n` +
            `HDR Supported: ${app.is_hdr_supported}\n`
        )
    }

    getHostId(): number {
        return this.hostId
    }
    getAppId(): number {
        return this.appId
    }

    mount(parent: HTMLElement): void {
        this.mounted++
        this.updateImage()

        parent.appendChild(this.divElement)
    }
    unmount(parent: HTMLElement): void {
        parent.removeChild(this.divElement)

        this.mounted--
        this.updateImage()
    }
}