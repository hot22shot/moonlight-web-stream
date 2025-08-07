import { Api, getApi, apiPutHost } from "./api.js";
import { AddHostModal } from "./component/host/add_modal.js";
import { HostList } from "./component/host/list.js";
import { Component, ComponentEvent } from "./component/index.js";
import { showErrorPopup } from "./component/error.js";
import { showModal } from "./component/modal/index.js";
import { setContextMenu } from "./component/context_menu.js";
import { GameList } from "./component/game/list.js";
import { Host } from "./component/host/index.js";
import { App } from "./api_bindings.js";

type AppState = { display: number | "hosts" }
function pushAppState(state: AppState) {
    history.pushState(state, "")
}

async function startApp() {
    const api = await getApi()

    const rootElement = document.getElementById("root");
    if (rootElement == null) {
        showErrorPopup("couldn't find root element", true)
        return;
    }

    const app = new MainApp(api)
    app.mount(rootElement)

    app.forceFetch()

    window.addEventListener("popstate", event => {
        app.setAppState(event.state)
    })
}

startApp()


class MainApp implements Component {
    private api: Api

    private divElement = document.createElement("div")

    private moonlightTextElement = document.createElement("h1")
    private actionElement = document.createElement("div")

    private gamesBackButton: HTMLButtonElement = document.createElement("button")

    private hostAddButton: HTMLButtonElement = document.createElement("button")

    private currentDisplay: "hosts" | "games" = "hosts"
    private hostList: HostList
    private gameList: GameList | null = null

    constructor(api: Api) {
        this.api = api

        // Moonlight text
        this.moonlightTextElement.innerHTML = "Moonlight Web"

        // Back button
        this.gamesBackButton.innerText = "Back"
        this.gamesBackButton.addEventListener("click", () => this.setCurrentGames(null))

        // Host add button
        this.hostAddButton.classList.add("host-add")
        this.hostAddButton.addEventListener("click", this.addHost.bind(this))

        // Host list
        this.hostList = new HostList(api)
        this.hostList.addHostOpenListener(this.onHostOpen.bind(this))

        // Append default elements
        this.divElement.appendChild(this.moonlightTextElement)
        this.divElement.appendChild(this.actionElement)
        this.hostList.mount(this.divElement)

        this.actionElement.appendChild(this.hostAddButton)

        // Context Menu
        document.body.addEventListener("contextmenu", this.onContextMenu.bind(this))

        pushAppState({ display: "hosts" })
    }

    setAppState(state: AppState) {
        if (state.display == "hosts") {
            this.setCurrentGames(null)
        } else {
            this.setCurrentGames(state.display)
        }
    }

    private async addHost() {
        const modal = new AddHostModal()

        let host = await showModal(modal);
        if (host) {
            const newHost = await apiPutHost(this.api, host)

            if (newHost) {
                this.hostList.insertList(newHost.host_id, newHost)
            } else {
                showErrorPopup("couldn't add host")
            }
        } else {
            showErrorPopup("couldn't add host")
        }
    }

    private onContextMenu(event: MouseEvent) {
        const elements = [
            {
                name: "Reload",
                callback: this.forceFetch.bind(this)
            }
        ]

        setContextMenu(event, {
            elements
        })
    }

    private async onHostOpen(event: ComponentEvent<Host>) {
        const hostId = event.component.getHostId()

        this.setCurrentGames(hostId)
    }
    private setCurrentGames(hostId: number | null, cache?: Array<App>) {
        // We want to transition to host view
        if (hostId == null) {

            // We aren't currently in host view
            if (this.currentDisplay == "games") {
                // Action elements
                this.actionElement.removeChild(this.gamesBackButton)
                this.actionElement.appendChild(this.hostAddButton)

                this.gameList?.unmount(this.divElement)
                this.hostList.mount(this.divElement)

                // Push new state
                pushAppState({ display: "hosts" })
            }

            this.currentDisplay = "hosts"
            return
        }

        // The old state is games
        if (this.currentDisplay == "games") {
            if (this.gameList?.getHostId() == hostId) {
                // If we're already in the correct state
                return
            } else {
                // If we're going to a different host
                this.gameList?.unmount(this.divElement)
            }
        }

        // Unmount host view if we're in the host view
        if (this.currentDisplay == "hosts") {
            // Action elements
            this.actionElement.appendChild(this.gamesBackButton)
            this.actionElement.removeChild(this.hostAddButton)

            this.hostList.unmount(this.divElement)

            pushAppState({ display: "hosts" })
        }

        // Mount game view
        this.gameList = new GameList(this.api, hostId, cache ?? null)
        this.gameList.mount(this.divElement)

        this.currentDisplay = "games"
    }

    async forceFetch() {
        await Promise.all([
            this.hostList.forceFetch(),
            this.gameList?.forceFetch()
        ])

        if (this.currentDisplay == "games" &&
            this.gameList &&
            !this.hostList.getHost(this.gameList.getHostId())) {
            // The newly fetched list doesn't contain the hosts game view we're in -> go to hosts
            this.setCurrentGames(null)
        }
    }

    mount(parent: HTMLElement): void {
        parent.appendChild(this.divElement)
    }
    unmount(parent: HTMLElement): void {
        parent.removeChild(this.divElement)
    }
}