import { mount } from 'svelte'
import './app.css'
import App from './App.svelte'
import { app } from './lib/stores/app.svelte'

const root = mount(App, {
  target: document.getElementById('app')!,
})

app.init();

export default root
