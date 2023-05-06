#!/bin/bash
echo 
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

# Check for flags
if [ $# -eq 0 ]; then
    help_message
    exit 1
fi

# Check if sqlx-cli is installed, and install it if not
if ! command -v sqlx > /dev/null; then
    echo "sqlx-cli not found. Installing..."
    cargo install sqlx-cli
else
    echo "sqlx-cli is already installed."
fi

echo 

ENV=".env" 
for arg in "$@"
do
    case $arg in
        --env)
            ENV=$1
            break
            ;;
        --clean)
            sqlx database drop -y
            sqlx database create

            shift
            ;;

    esac
done

export $(cat $ENV | xargs)
for arg in "$@"
do
    case $arg in
        --help)
            help_message
            exit 0
            ;;

        --create)
            sqlx database create
            shift
            ;;
        --prod)
            sqlx migrate run
            shift
            ;;
        --dev)
            echo "CAUTION: creating development database..."
            sqlx migrate run --ignore-missing
            sqlx migrate run --ignore-missing --source migrations/test_migrations
            shift
            ;;
        *)
            echo "Unknown option: $arg"
            help_message
            exit 1
            ;;
    esac
done

echo
