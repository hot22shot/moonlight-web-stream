import { Component } from "../index.js";
import { Api, apiPatchUser } from "../../api.js";
import { DetailedUser, PatchUserRequest, UserRole } from "../../api_bindings.js";
import { InputComponent, SelectComponent } from "../input.js";
import { createSelectRoleInput } from "./role_select.js";

export class DetailedUserPage implements Component {

    private api: Api

    private root = document.createElement("div")

    private id

    private idElement: InputComponent
    private name: InputComponent
    private role: SelectComponent

    private applyButton = document.createElement("button")

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
    }

    private async apply() {
        const request: PatchUserRequest = {
            id: this.id,
            role: this.role.getValue() as UserRole,
            password: null, // TODO: change password
        };

        await apiPatchUser(this.api, request)
    }

    mount(parent: HTMLElement): void {
        parent.appendChild(this.root)
    }
    unmount(parent: HTMLElement): void {
        parent.removeChild(this.root)
    }
}