import { invoke } from '@tauri-apps/api/core';
import type { Project, Collection, Entry, Page } from './types';
export const listProjects = () => invoke<Project[]>('list_projects');
export const getProject = (id: string) => invoke<Project>('get_project', { id });
export const listCollections = (projectId: string) => invoke<Collection[]>('list_collections', { projectId });
export const getCollection = (id: string) => invoke<Collection>('get_collection', { id });
export const getEntry = (id: string) => invoke<Entry>('get_entry', { id });
export const listPages = (entryId: string) => invoke<Page[]>('list_pages', { entryId });
