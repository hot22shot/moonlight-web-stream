import { Component, ComponentEvent } from "../index.js";
import { Api, apiDeleteUser, apiPatchUser } from "../../api.js";
import { DetailedUser, PatchUserRequest, UserRole } from "../../api_bindings.js";
import { InputComponent, SelectComponent } from "../input.js";
import { createSelectRoleInput } from "./role_select.js";
import { tryDeleteUser, UserEventListener } from "./index.js";

export class DetailedUserPage implements Component {

    private api: Api

    private root = document.createElement("div")

    private id

    private idElement: InputComponent
    private name: InputComponent
    private role: SelectComponent

    private applyButton = document.createElement("button")
    private deleteButton = document.createElement("button")

    constructor(api: Api, user: DetailedUser) {
        this.api = api
        this.id = user.id

        this.root.classList.add("user-info")

        this.idElement = new InputComponent("userId", "number", "User Id", {
            defaultValue: `${user.id}`
        })
        this.idElement.setEnabled(false)
        this.idElement.mount(this.root)

        this.name = new InputComponent("userName", "text", "User Name", {
            defaultValue: user.name,
        })
        this.name.setEnabled(false)
        this.name.mount(this.root)

        this.role = createSelectRoleInput(user.role)
        this.role.mount(this.root)

        this.applyButton.addEventListener("click", this.apply.bind(this))
        this.applyButton.innerText = "Apply"
        this.root.appendChild(this.applyButton)

        this.deleteButton.addEventListener("click", this.delete.bind(this))
        this.deleteButton.classList.add("user-info-delete")
        this.deleteButton.innerText = "Delete"
        this.root.appendChild(this.deleteButton)
    }

    private async apply() {
        const request: PatchUserRequest = {
            id: this.id,
            role: this.role.getValue() as UserRole,
            password: null, // TODO: change password
        };

        await apiPatchUser(this.api, request)
    }

    private async delete() {
        await tryDeleteUser(this.api, this.id)

        this.root.dispatchEvent(new ComponentEvent("ml-userdeleted", this))
    }

    addDeletedListener(listener: UserEventListener, options?: EventListenerOptions) {
        this.root.addEventListener("ml-userdeleted", listener as any, options)
    }
    removeDeletedListener(listener: UserEventListener) {
        this.root.removeEventListener("ml-userdeleted", listener as any)
    }

    getUserId(): number {
        return this.id
    }

    mount(parent: HTMLElement): void {
        parent.appendChild(this.root)
    }
    unmount(parent: HTMLElement): void {
        parent.removeChild(this.root)
    }
}