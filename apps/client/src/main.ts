import './theme';
import '@honkoku/ui/tokens.css';
import './style.css';
import { mount } from 'svelte';
import App from './App.svelte';
const target = document.getElementById('app');
if (!target) throw new Error('Application mount point is missing');
mount(App, { target });
