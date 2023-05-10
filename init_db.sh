#!/usr/bin/env bash

help_message() {
    echo
    echo "Usage: $0 [OPTIONS]"
    echo "Manage database with the following options:"
    echo
    echo "  --help      Show this help message"
    echo "  --clean     Drop and recreate the database"
    echo "  --create    Create the database"
    echo "  --prod      Run migrations for the production database"
    echo "  --dev       Run migrations for the development database, including test migrations"
    echo
    echo "Example:"
    echo "clean dev db initialization: ./init_db.sh --clean --dev"
    echo
}

FLAGS=(
    "--clean"
    "--create"
    "--prod"
    "--dev"
)

FUNCTIONS=(
    "clean_database"
    "create_database"
    "migrate_prod"
    "migrate_dev"
)

clean_database() {
    sqlx database drop -y
    sqlx database create
}

create_database() {
    sqlx database create
}

migrate_prod() {
    sqlx migrate run
}

migrate_dev() {
    echo "CAUTION: creating development database..."
    sqlx migrate run --ignore-missing
    sqlx migrate run --ignore-missing --source migrations/test_migrations
}

check_sqlx() {
    if ! command -v sqlx > /dev/null; then
        echo "sqlx-cli not found. Installing..."
        cargo install sqlx-cli
    else
        echo "sqlx-cli is already installed."
    fi
}

if [ $# -eq 0 ]; then
    help_message
    exit 1
fi

ENV=".env"
args=("$@")

STATUS=()
for _ in "${FLAGS[@]}"; do
    STATUS+=(false)
done

while (( "$#" )); do
    case $1 in
        --env)
            ENV="$2"
            shift 2
            ;;
        --help)
            help_message
            exit 0
            ;;
        *)
            found=false
            for i in "${!FLAGS[@]}"; do
                if [ "$1" = "${FLAGS[$i]}" ]; then
                    STATUS[$i]=true
                    found=true
                    shift
                    break
                fi
            done
            if [ "$found" = false ]; then
                echo "Unknown option: $1"
                help_message
                exit 1
            fi
            ;;
    esac
done

check_sqlx

export $(grep -v '^#' "$ENV" | xargs)

for i in "${!STATUS[@]}"; do
    if [ "${STATUS[$i]}" = true ]; then
        ${FUNCTIONS[$i]}
    fi
done

echo
