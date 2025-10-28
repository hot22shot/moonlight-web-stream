import { Api, apiGetUser } from "../../api.js";
import { DetailedUser } from "../../api_bindings.js";
import { Component } from "../index.js";

export class User implements Component {

    private api: Api

    private user: DetailedUser | { id: number }

    private div = document.createElement("div")
    private nameElement = document.createElement("p")

    constructor(api: Api, user: DetailedUser | { id: number }) {
        this.api = api

        this.div.appendChild(this.nameElement)

        this.user = user
        if ("name" in user) {
            this.updateCache(user)
        } else {
            this.forceFetch()
        }
    }

    async forceFetch() {
        const user = await apiGetUser(this.api, {
            name: null,
            user_id: this.user.id,
        })

        this.updateCache(user)
    }
    updateCache(user: DetailedUser) {
        this.user = user

        this.nameElement.innerText = user.name
    }

    getCache(): DetailedUser | null {
        if ("name" in this.user) {
            return this.user
        } else {
            return null
        }
    }

    getUserId(): number {
        return this.user.id
    }

    mount(parent: HTMLElement): void {
        parent.appendChild(this.div)
    }
    unmount(parent: HTMLElement): void {
        parent.removeChild(this.div)
    }
}