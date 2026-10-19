# Rlang

## Overview

### Short introduction

In my past projects I have not documented anything until much later on, and I have not enjoyed the process of having to go back and remember what I did. This time I plan on keeping a steady log of what is going on.

As of right now, I find it noteworthy that AI has allowed for extremely fast creation and iteration of the basic idea. I have used these iterations as the basis for learning Rust itself. The learning process is orders of magnitudes quicker as a result. 

### What is Rlang?

Rlang (Rapid Language) is a project coded in rust, created with heavy AI assistance, with the goal of allowing the rapid creation of languages and git-like versioning over many languages. The primary aim is to treat languages as disposable and allow for rapid iteration, creation, forking, sharing, and more. This is the first step in a communication software toolkit that I plan to build over time. The second steps will introduce language based networks with the goal of creating communities rapidly creating novel languages, sharing them with each other, and finally, communicating with them to allow for an accelerated development in linguistic evolution and human communication

### Versioning and usage

Currently, the most up-to-date version is the directory titled `langsmithV2`. Simply navigate to `~/Rlang/langsmithV2/langsmith/target/release`, and enter the command `export PATH=$PATH:$(pwd)` To add the langsmith command to your path for the current session. You may also use previous versions.

Use the `langsmith` command to list all the available commands that can be used. 

### Displaying symbols

Displaying symbols in the terminal is one of the key intended features of the program. This can be done with chafa. As chafa is a third party tool that must be installed seperately from this project I cannot guarantee how it will work on other machines or local environments. 