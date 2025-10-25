#!/usr/bin/env node

import { execSync } from 'node:child_process';
import { readFileSync, writeFileSync, existsSync, mkdirSync, cpSync, rmSync } from 'node:fs';
import { join } from 'node:path';
import { EXAMPLE_METADATA } from './examplesData.mjs';

const REPO_URL = 'https://github.com/rivet-dev/rivetkit.git';
const BRANCH = 'main';
const TEMP_DIR = '/tmp/rivetkit-examples';
const TEMP_EXAMPLE_DIR = '/tmp/rivet-example-temp';
const OUTPUT_DIR = './src/data/examples';
const OUTPUT_FILE = 'examples.ts';

// Ensure output directory exists
if (!existsSync(OUTPUT_DIR)) {
  mkdirSync(OUTPUT_DIR, { recursive: true });
}

// Clone or update the repository
function updateRepo() {
  if (existsSync(TEMP_DIR)) {
    console.log('Updating existing repository...');
    execSync('git clean -fd', { cwd: TEMP_DIR });
    execSync('git reset --hard', { cwd: TEMP_DIR });
    execSync('git fetch origin', { cwd: TEMP_DIR });
    execSync(`git checkout ${BRANCH}`, { cwd: TEMP_DIR });
    execSync(`git pull origin ${BRANCH}`, { cwd: TEMP_DIR });
  } else {
    console.log('Cloning repository...');
    execSync(`git clone -b ${BRANCH} ${REPO_URL} ${TEMP_DIR}`);
  }
}

// Replace workspace dependencies with version numbers
function replaceWorkspaceDependencies(content) {
  return content
    .replace(/@rivetkit\/([^"]+)": "workspace:\*"/g, '@rivetkit/$1": "^0.9.1"')
    .replace(/"workspace:\*"/g, '"^0.9.1"');
}

// Get only the examples defined in metadata
function getExamplesToProcess() {
  const examplesDir = join(TEMP_DIR, 'examples');
  
  if (!existsSync(examplesDir)) {
    throw new Error('Examples directory not found');
  }
  
  const definedExamples = Object.keys(EXAMPLE_METADATA);
  const availableExamples = [];
  
  // Check which defined examples actually exist in the repository
  for (const exampleName of definedExamples) {
    const examplePath = join(examplesDir, exampleName);
    if (existsSync(examplePath)) {
      availableExamples.push(exampleName);
    } else {
      throw new Error(`Example defined in metadata but not found in repo: ${exampleName}`);
    }
  }
  
  console.log(`Processing ${availableExamples.length} examples: ${availableExamples.join(', ')}`);
  return availableExamples;
}

// Copy example to temp folder, install dependencies, then process files
function processExample(exampleName) {
  const exampleDir = join(TEMP_DIR, 'examples', exampleName);
  
  if (!existsSync(exampleDir)) {
    throw new Error(`Example directory not found: ${exampleName}`);
  }

  // Create unique temp folder for this example
  const tempExampleDir = join(TEMP_EXAMPLE_DIR, exampleName);
  
  // Clean up any existing temp folder
  if (existsSync(tempExampleDir)) {
    rmSync(tempExampleDir, { recursive: true, force: true });
  }
  
  // Copy example to temp folder
  console.log(`Copying ${exampleName} to temp folder...`);
  cpSync(exampleDir, tempExampleDir, { recursive: true });
  
  // Replace workspace dependencies in package.json before npm install
  const packageJsonPath = join(tempExampleDir, 'package.json');
  if (existsSync(packageJsonPath)) {
    const packageJsonContent = readFileSync(packageJsonPath, 'utf-8');
    const updatedPackageJson = replaceWorkspaceDependencies(packageJsonContent);
    writeFileSync(packageJsonPath, updatedPackageJson);
  }
  
  // Run npm install to generate lockfile
  console.log(`Running npm install for ${exampleName}...`);
  try {
    execSync('npm install', { 
      cwd: tempExampleDir,
      stdio: 'inherit' 
    });
  } catch (error) {
    throw new Error(`npm install failed for ${exampleName}: ${error.message}`);
  }
  
  // Remove node_modules after npm install
  console.log(`Removing node_modules for ${exampleName}...`);
  const nodeModulesPath = join(tempExampleDir, 'node_modules');
  if (existsSync(nodeModulesPath)) {
    rmSync(nodeModulesPath, { recursive: true, force: true });
  }

  const files = {};
  
  try {
    // Get all files recursively in the directory, including subdirectories
    const allFiles = execSync('find . -type f -not -path "*/.git/*"', { 
      cwd: tempExampleDir,
      encoding: 'utf-8'
    }).trim().split('\n');
    
    for (const file of allFiles) {
      if (file && file !== '.') {
        const filePath = join(tempExampleDir, file);
        const relativePath = file.replace('./', '');
        
        // Exclude turbo.json from bundled files
        if (relativePath === 'turbo.json' || relativePath === 'rivet.json') {
          continue;
        }
        
        if (existsSync(filePath)) {
          try {
            let content = readFileSync(filePath, 'utf-8');
            
            // Replace workspace dependencies in package.json files
            if (relativePath.endsWith('package.json')) {
              content = replaceWorkspaceDependencies(content);
            }
            
            files[relativePath] = content;
          } catch (readError) {
            // Skip binary files or files that can't be read as text
            throw new Error(`Failed to read file ${relativePath}: ${readError.message}`);
          }
        }
      }
    }
    
    // Clean up temp folder
    if (existsSync(tempExampleDir)) {
      rmSync(tempExampleDir, { recursive: true, force: true });
    }
    
    console.log(`Found ${Object.keys(files).length} files in ${exampleName}`);
    return files;
  } catch (error) {
    // Clean up temp folder in case of error
    if (existsSync(tempExampleDir)) {
      rmSync(tempExampleDir, { recursive: true, force: true });
    }
    throw new Error(`Error reading files from ${exampleName}: ${error.message}`);
  }
}

// Main function
function main() {
  console.log('Generating examples...');
  
  // Update the repository
  updateRepo();
  
  // Ensure temp example directory exists
  if (!existsSync(TEMP_EXAMPLE_DIR)) {
    mkdirSync(TEMP_EXAMPLE_DIR, { recursive: true });
  }
  
  // Get examples to process (only those defined in metadata)
  const exampleNames = getExamplesToProcess();
  
  if (exampleNames.length === 0) {
    console.error('No examples found in the repository');
    return;
  }
  
  const generatedExamples = [];
  
  try {
    // Process each example
    for (const exampleName of exampleNames) {
      console.log(`Processing example: ${exampleName}`);
      const files = processExample(exampleName);
      
      if (files && Object.keys(files).length > 0) {
        // Create example object with metadata and files
        const exampleData = {
          id: exampleName,
          ...EXAMPLE_METADATA[exampleName],
          files: files
        };
        
        generatedExamples.push(exampleData);
        console.log(`✓ Processed ${exampleName} with ${Object.keys(files).length} files`);
      } else {
        throw new Error(`No files found for example: ${exampleName}`);
      }
    }
    
    // Generate TypeScript file content
    const tsContent = `// This file was generated by scripts/generateExamples.mjs
// Do not edit this file directly - it will be overwritten

export type StateTypeTab = "memory" | "sqlite";

export interface ExampleData {
  id: string;
  icon: string;
  title: string;
  filesToOpen: string[];
  tab: StateTypeTab;
  files: Record<string, string>;
}

export const examples: ExampleData[] = ${JSON.stringify(generatedExamples, null, 2)};
`;

    // Write the generated examples to a TypeScript file
    const outputPath = join(OUTPUT_DIR, OUTPUT_FILE);
    writeFileSync(outputPath, tsContent);
    
    console.log(`✓ Generated ${OUTPUT_FILE} with ${generatedExamples.length} examples`);
    console.log('Examples generation complete!');
  } finally {
    // Clean up temp example directory
    if (existsSync(TEMP_EXAMPLE_DIR)) {
      rmSync(TEMP_EXAMPLE_DIR, { recursive: true, force: true });
    }
  }
}

main();
